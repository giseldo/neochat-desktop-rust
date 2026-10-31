mod chat;
mod composer;
mod settings;
mod sidebar;

use crate::mistral::{Message, MistralClient, TokenUsage};
use crate::storage::{ truncate_chars, Store, StoredMessage, ThemeChoice};
use crate::{secrets, theme};
use eframe::egui;
use egui::Context;
use egui_commonmark::CommonMarkCache;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::Instant;

pub const SYSTEM_PROMPT: &str =
    "Você é o NeoChat, um assistente prestativo, claro e conciso. Responda no idioma do usuário.";

pub enum Event {
    Token(String),
    Done(Option<TokenUsage>),
    Error(String),
}

pub struct NeoChat {
    rt: tokio::runtime::Runtime,
    pub(super) store: Store,
    pub(super) api_key: String,
    pub(super) input: String,
    pub(super) partial: String,
    pub(super) rx: Option<Receiver<Event>>,
    pub(super) cancel: Option<Arc<AtomicBool>>,
    pub(super) request_started_at: Option<Instant>,
    pub(super) streamed_tokens: u32,
    pub(super) error: Option<String>,
    pub(super) status: Option<String>,
    pub(super) settings_open: bool,
    pub(super) settings_key: String,
    pub(super) settings_model: String,
    pub(super) settings_theme: ThemeChoice,
    pub(super) settings_temperature: f32,
    pub(super) settings_max_tokens: String,
    pub(super) settings_prompt_caching: bool,
    pub(super) login_rx: Option<Receiver<Result<String, String>>>,
    pub(super) login_pending: bool,
    pub(super) login_status: Option<(bool, String)>,
    pub(super) test_rx: Option<Receiver<Result<String, String>>>,
    pub(super) test_pending: bool,
    pub(super) test_status: Option<(bool, String)>,
    pub(super) models_rx: Option<Receiver<Result<Vec<String>, String>>>,
    pub(super) models_pending: bool,
    pub(super) models_status: Option<(bool, String)>,
    pub(super) rename: Option<(String, String)>,
    pub(super) rename_focus: bool,
    pub(super) delete: Option<String>,
    pub(super) markdown_cache: CommonMarkCache,
    applied_theme: Option<ThemeChoice>,
    applied_font_scale: Option<f32>,
    last_window_size: Option<[f32; 2]>,
    centered: bool,
    pub(super) follow_output: bool,
    pub(super) jump_to_bottom: bool,
    pub(super) focus_composer: bool,
    pub(super) sidebar_collapsed: bool,
    pub(super) copied_feedback: Option<(usize, Instant)>,
}

impl NeoChat {
    pub fn new(cc: &eframe::CreationContext<'_>, mut store: Store) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, store.config.theme);
        cc.egui_ctx.set_zoom_factor(store.config.font_scale);

        // Remove conversas vazias duplicadas acumuladas, mantendo apenas a ativa (ou uma única)
        let active_id = store.conversations.active.clone();
        let has_non_empty = store
            .conversations
            .items
            .iter()
            .any(|c| !c.messages.is_empty());
        if has_non_empty {
            store
                .conversations
                .items
                .retain(|c| !c.messages.is_empty() || Some(c.id.as_str()) == active_id.as_deref());
        } else if store.conversations.items.len() > 1 {
            let keep_id = active_id.or_else(|| store.conversations.items.first().map(|c| c.id.clone()));
            store
                .conversations
                .items
                .retain(|c| Some(c.id.as_str()) == keep_id.as_deref());
        }
        if store.conversations.active().is_none() {
            store.conversations.active = store.conversations.items.first().map(|c| c.id.clone());
        }

        let api_key = std::env::var("MISTRAL_API_KEY")
            .ok()
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
            .or_else(secrets::load_api_key)
            .unwrap_or_default();

        let settings_model = store.config.model.clone();
        let settings_theme = store.config.theme;
        let settings_temperature = store.config.temperature;
        let settings_max_tokens = store
            .config
            .max_tokens
            .map(|m| m.to_string())
            .unwrap_or_default();
        let settings_prompt_caching = store.config.prompt_caching;

        let mut app = Self {
            rt: tokio::runtime::Runtime::new().expect("falha ao criar o runtime tokio"),
            store,
            api_key,
            input: String::new(),
            partial: String::new(),
            rx: None,
            cancel: None,
            request_started_at: None,
            streamed_tokens: 0,
            error: None,
            status: None,
            settings_open: false,
            settings_key: String::new(),
            settings_model,
            settings_theme,
            settings_temperature,
            settings_max_tokens,
            settings_prompt_caching,
            login_rx: None,
            login_pending: false,
            login_status: None,
            test_rx: None,
            test_pending: false,
            test_status: None,
            models_rx: None,
            models_pending: false,
            models_status: None,
            rename: None,
            rename_focus: false,
            delete: None,
            markdown_cache: CommonMarkCache::default(),
            applied_theme: None,
            applied_font_scale: None,
            last_window_size: None,
            centered: false,
            follow_output: true,
            jump_to_bottom: false,
            focus_composer: false,
            sidebar_collapsed: false,
            copied_feedback: None,
        };
        if app.has_auth() && app.store.config.cached_models.is_empty() {
            app.start_fetch_models(&cc.egui_ctx);
        }
        app
    }

    pub(super) fn toggle_sidebar(&mut self) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
    }

    pub(super) fn adjust_font_scale(&mut self, delta: f32) {
        let next = (((self.store.config.font_scale + delta) * 10.0).round() / 10.0).clamp(0.8, 1.6);
        if (next - self.store.config.font_scale).abs() < 0.001 {
            return;
        }
        self.store.config.font_scale = next;
        self.save();
    }

    pub(super) fn reset_font_scale(&mut self) {
        if (self.store.config.font_scale - 1.0).abs() < 0.001 {
            return;
        }
        self.store.config.font_scale = 1.0;
        self.save();
    }

    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    pub(super) fn has_auth(&self) -> bool {
        !self.api_key.trim().is_empty()
    }

    pub(super) fn start_login(&mut self, ctx: &Context) {
        if self.login_pending {
            return;
        }
        let (tx, rx) = channel();
        let ctx_task = ctx.clone();
        self.login_rx = Some(rx);
        self.login_pending = true;
        self.login_status = None;
        self.rt.spawn(async move {
            let result = crate::oauth::login()
                .await
                .map_err(|err| format!("{err:#}"));
            tx.send(result).ok();
            ctx_task.request_repaint();
        });
    }

    pub(super) fn poll_login(&mut self) {
        let Some(rx) = self.login_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(api_key)) => {
                self.login_pending = false;
                self.api_key = api_key;
                self.settings_key = self.api_key.clone();
                if let Err(err) = secrets::save_api_key(&self.api_key) {
                    self.login_status = Some((false, format!("Falha ao salvar a chave: {err:#}")));
                } else {
                    self.login_status =
                        Some((true, "Login concluido; chave da API salva.".to_string()));
                }
            }
            Ok(Err(err)) => {
                self.login_pending = false;
                self.login_status = Some((false, err));
            }
            Err(TryRecvError::Empty) => {
                self.login_rx = Some(rx);
            }
            Err(TryRecvError::Disconnected) => {
                self.login_pending = false;
                self.login_status =
                    Some((false, "Processo de login encerrado inesperadamente.".to_string()));
            }
        }
    }

    pub(super) fn model_list(&self) -> Vec<String> {
        let mut models: Vec<String> = if self.store.config.cached_models.is_empty() {
            crate::storage::MODELS
                .iter()
                .map(|m| m.to_string())
                .collect()
        } else {
            self.store.config.cached_models.clone()
        };
        if !self.settings_model.trim().is_empty() && !models.contains(&self.settings_model) {
            models.insert(0, self.settings_model.clone());
        }
        if !self.store.config.model.trim().is_empty() && !models.contains(&self.store.config.model) {
            models.insert(0, self.store.config.model.clone());
        }
        models
    }

    fn ensure_active(&mut self) {
        if self.store.conversations.active().is_none() {
            self.store.conversations.new_conversation();
        }
    }

    fn save(&self) {
        self.store.save();
    }

    pub(super) fn open_settings(&mut self) {
        self.settings_key = self.api_key.clone();
        self.settings_model = self.store.config.model.clone();
        self.settings_theme = self.store.config.theme;
        self.settings_temperature = self.store.config.temperature;
        self.settings_max_tokens = self
            .store
            .config
            .max_tokens
            .map(|m| m.to_string())
            .unwrap_or_default();
        self.settings_prompt_caching = self.store.config.prompt_caching;
        self.test_status = None;
        self.models_status = None;
        self.settings_open = true;
    }

    pub(super) fn new_conversation(&mut self) {
        if self.busy() {
            self.stop();
        }
        if let Some(existing_empty) = self
            .store
            .conversations
            .items
            .iter_mut()
            .find(|c| c.messages.is_empty())
        {
            existing_empty.touch();
            let id = existing_empty.id.clone();
            self.store.conversations.active = Some(id);
        } else {
            self.store.conversations.new_conversation();
        }
        self.input.clear();
        self.partial.clear();
        self.error = None;
        self.status = None;
        self.follow_output = true;
        self.focus_composer = true;
        self.save();
    }

    pub(super) fn select_conversation(&mut self, id: &str) {
        if self.store.conversations.active.as_deref() == Some(id) {
            return;
        }
        if self.busy() {
            self.stop();
        }
        let target_id = id.to_string();
        self.store
            .conversations
            .items
            .retain(|c| !c.messages.is_empty() || c.id == target_id);
        self.store.conversations.active = Some(target_id);
        self.partial.clear();
        self.error = None;
        self.status = None;
        self.follow_output = true;
        self.jump_to_bottom = true;
        self.focus_composer = true;
        self.save();
    }

    pub(super) fn delete_conversation(&mut self, id: &str) {
        if self.busy() {
            self.stop();
        }
        self.store.conversations.remove(id);
        if self.store.conversations.items.is_empty() {
            self.store.conversations.new_conversation();
        }
        self.save();
    }

    fn push_assistant(&mut self, content: String, interrupted: bool, usage: Option<TokenUsage>) {
        let duration_ms = self
            .request_started_at
            .take()
            .map(|t| t.elapsed().as_millis() as u64)
            .filter(|&ms| ms > 0);
        let fallback_tokens = (self.streamed_tokens > 0).then_some(self.streamed_tokens);
        self.streamed_tokens = 0;

        if let Some(conversation) = self.store.conversations.active_mut() {
            let mut message = StoredMessage::new("assistant", content);
            message.interrupted = interrupted;
            message.duration_ms = duration_ms;
            if let Some(u) = usage {
                message.prompt_tokens = (u.prompt_tokens > 0).then_some(u.prompt_tokens);
                message.completion_tokens = (u.completion_tokens > 0)
                    .then_some(u.completion_tokens)
                    .or(fallback_tokens);
                message.total_tokens = (u.total_tokens > 0).then_some(u.total_tokens);
                let cached = u.cached_tokens();
                message.cached_tokens = (cached > 0).then_some(cached);
            } else {
                message.completion_tokens = fallback_tokens;
            }
            conversation.messages.push(message);
            conversation.touch();
        }
        self.save();
    }

    pub(super) fn send(&mut self, ctx: &Context) {
        let text = self.input.trim().to_string();
        if text.is_empty() || self.busy() {
            return;
        }
        if !self.has_auth() {
            self.error = Some(
                "Faça login com a conta Mistral ou informe a chave da API nas configurações."
                    .to_string(),
            );
            self.open_settings();
            return;
        }

        self.input.clear();
        self.error = None;
        self.status = None;
        self.partial.clear();
        self.follow_output = true;
        self.jump_to_bottom = true;
        self.focus_composer = true;

        self.ensure_active();
        if let Some(conversation) = self.store.conversations.active_mut() {
            conversation.messages.push(StoredMessage::new("user", text));
            conversation.touch();
            conversation.ensure_title();
        }
        self.save();
        self.start_request(ctx);
    }

    fn start_request(&mut self, ctx: &Context) {
        let Some(conversation) = self.store.conversations.active() else {
            return;
        };
        if conversation.messages.is_empty() {
            return;
        }

        let mut history = vec![Message::new("system", SYSTEM_PROMPT)];
        history.extend(conversation.to_api_messages());

        let cache_key = if self.store.config.prompt_caching {
            Some(conversation.id.clone())
        } else {
            None
        };

        let client = MistralClient::new(
            self.api_key.trim().to_string(),
            self.store.config.model.clone(),
        )
        .with_params(
            Some(self.store.config.temperature),
            self.store.config.max_tokens,
        )
        .with_prompt_cache_key(cache_key);
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = Arc::clone(&cancel);
        let ctx_task = ctx.clone();

        self.rx = Some(rx);
        self.cancel = Some(cancel);
        self.partial.clear();
        self.request_started_at = Some(Instant::now());
        self.streamed_tokens = 0;

        self.rt.spawn(async move {
            let tx_token = tx.clone();
            let ctx_token = ctx_task.clone();
            let result = client
                .chat_stream_cancellable(&history, Some(&cancel_task), move |token| {
                    tx_token.send(Event::Token(token.to_string())).ok();
                    ctx_token.request_repaint();
                })
                .await;
            match result {
                Ok((_, usage)) => {
                    tx.send(Event::Done(usage)).ok();
                }
                Err(err) => {
                    tx.send(Event::Error(format!("{err:#}"))).ok();
                }
            }
            ctx_task.request_repaint();
        });
    }

    pub(super) fn stop(&mut self) {
        if let Some(flag) = self.cancel.take() {
            flag.store(true, Ordering::Relaxed);
        }
        let reply = std::mem::take(&mut self.partial);
        if !reply.trim().is_empty() {
            self.push_assistant(reply, true, None);
        } else {
            self.request_started_at = None;
            self.streamed_tokens = 0;
        }
        self.rx = None;
        self.follow_output = true;
        self.jump_to_bottom = true;
        self.status = Some("Resposta interrompida.".to_string());
    }

    pub(super) fn regenerate(&mut self, ctx: &Context) {
        if self.busy() {
            return;
        }
        if !self.has_auth() {
            self.error = Some(
                "Faça login com a conta Mistral ou informe a chave da API nas configurações."
                    .to_string(),
            );
            self.open_settings();
            return;
        }

        let Some(conversation) = self.store.conversations.active_mut() else {
            return;
        };
        while conversation
            .messages
            .last()
            .map(|m| !m.is_user())
            .unwrap_or(false)
        {
            conversation.messages.pop();
        }
        if conversation.messages.is_empty() {
            return;
        }
        conversation.touch();

        self.error = None;
        self.status = None;
        self.partial.clear();
        self.follow_output = true;
        self.jump_to_bottom = true;
        self.save();
        self.start_request(ctx);
    }

    fn poll_events(&mut self) {
        let Some(rx) = self.rx.take() else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(Event::Token(token)) => {
                    self.partial.push_str(&token);
                    self.streamed_tokens = self.streamed_tokens.saturating_add(1);
                    self.follow_output = true;
                }
                Ok(Event::Done(usage)) => {
                    let reply = std::mem::take(&mut self.partial);
                    if !reply.trim().is_empty() {
                        self.push_assistant(reply, false, usage);
                    } else {
                        self.request_started_at = None;
                        self.streamed_tokens = 0;
                    }
                    self.cancel = None;
                    self.rx = None;
                    return;
                }
                Ok(Event::Error(err)) => {
                    self.error = Some(err);
                    let reply = std::mem::take(&mut self.partial);
                    if !reply.trim().is_empty() {
                        self.push_assistant(reply, true, None);
                    } else {
                        self.request_started_at = None;
                        self.streamed_tokens = 0;
                    }
                    self.cancel = None;
                    self.rx = None;
                    return;
                }
                Err(TryRecvError::Empty) => {
                    self.rx = Some(rx);
                    return;
                }
                Err(TryRecvError::Disconnected) => {
                    let reply = std::mem::take(&mut self.partial);
                    if !reply.trim().is_empty() {
                        self.push_assistant(reply, true, None);
                    } else {
                        self.request_started_at = None;
                        self.streamed_tokens = 0;
                    }
                    self.cancel = None;
                    self.rx = None;
                    return;
                }
            }
        }
    }

    pub(super) fn poll_test(&mut self) {
        let Some(rx) = self.test_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.test_pending = false;
                self.test_status = Some(match result {
                    Ok(message) => (true, message),
                    Err(err) => (false, err),
                });
            }
            Err(TryRecvError::Empty) => {
                self.test_rx = Some(rx);
            }
            Err(TryRecvError::Disconnected) => {
                self.test_pending = false;
                self.test_status = Some((false, "Teste cancelado.".to_string()));
            }
        }
    }

    pub(super) fn start_test(&mut self, ctx: &Context) {
        let client = MistralClient::new(
            self.settings_key.trim().to_string(),
            self.settings_model.clone(),
        );
        let (tx, rx) = channel();
        let ctx_task = ctx.clone();
        self.test_rx = Some(rx);
        self.test_pending = true;
        self.test_status = None;
        self.rt.spawn(async move {
            let result = client
                .test_connection()
                .await
                .map_err(|err| format!("{err:#}"));
            tx.send(result).ok();
            ctx_task.request_repaint();
        });
    }

    pub(super) fn start_fetch_models(&mut self, ctx: &Context) {
        if self.models_pending {
            return;
        }
        let key = if self.settings_open && !self.settings_key.trim().is_empty() {
            self.settings_key.trim().to_string()
        } else {
            self.api_key.trim().to_string()
        };
        let client = MistralClient::new(key, self.settings_model.clone());
        let (tx, rx) = channel();
        let ctx_task = ctx.clone();
        self.models_rx = Some(rx);
        self.models_pending = true;
        self.models_status = None;
        self.rt.spawn(async move {
            let result = client
                .list_models()
                .await
                .map_err(|err| format!("{err:#}"));
            tx.send(result).ok();
            ctx_task.request_repaint();
        });
    }

    pub(super) fn poll_models(&mut self) {
        let Some(rx) = self.models_rx.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(Ok(models)) => {
                self.models_pending = false;
                let count = models.len();
                self.store.config.cached_models = models;
                self.store.save();
                self.models_status = Some((
                    true,
                    format!("{count} modelos carregados da API da Mistral."),
                ));
            }
            Ok(Err(err)) => {
                self.models_pending = false;
                self.models_status = Some((false, err));
            }
            Err(TryRecvError::Empty) => {
                self.models_rx = Some(rx);
            }
            Err(TryRecvError::Disconnected) => {
                self.models_pending = false;
                self.models_status = Some((false, "Falha ao carregar modelos.".to_string()));
            }
        }
    }

    pub(super) fn apply_settings(&mut self) {
        self.api_key = self.settings_key.trim().to_string();
        if let Err(err) = secrets::save_api_key(&self.api_key) {
            self.error = Some(format!("Falha ao salvar a chave: {err:#}"));
        }
        self.store.config.model = self.settings_model.clone();
        self.store.config.theme = self.settings_theme;
        self.store.config.temperature =
            ((self.settings_temperature.clamp(0.0, 1.5) * 100.0).round()) / 100.0;
        self.store.config.max_tokens = self
            .settings_max_tokens
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|&m| m > 0);
        self.store.config.prompt_caching = self.settings_prompt_caching;
        self.store.save();
        self.settings_open = false;
        self.status = Some("Configurações salvas.".to_string());
        self.error = None;
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        let (new_chat, settings, escape, toggle_sidebar, font_up, font_down, font_reset) =
            ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::N) && i.modifiers.command,
                    i.key_pressed(egui::Key::Comma) && i.modifiers.command,
                    i.key_pressed(egui::Key::Escape),
                    i.key_pressed(egui::Key::B) && i.modifiers.command,
                    i.key_pressed(egui::Key::Equals) && i.modifiers.command,
                    i.key_pressed(egui::Key::Minus) && i.modifiers.command,
                    i.key_pressed(egui::Key::Num0) && i.modifiers.command,
                )
            });
        if new_chat {
            self.new_conversation();
        }
        if settings {
            self.open_settings();
        }
        if toggle_sidebar {
            self.toggle_sidebar();
        }
        if font_up {
            self.adjust_font_scale(0.1);
        }
        if font_down {
            self.adjust_font_scale(-0.1);
        }
        if font_reset {
            self.reset_font_scale();
        }
        if escape && self.busy() {
            self.stop();
        }
    }

    fn track_window_size(&mut self, ctx: &Context) {
        let (rect, monitor, maximized, fullscreen) = ctx.input(|i| {
            let viewport = i.viewport();
            (
                viewport.inner_rect,
                viewport.monitor_size,
                viewport.maximized.unwrap_or(false),
                viewport.fullscreen.unwrap_or(false),
            )
        });

        if maximized || fullscreen {
            return;
        }

        let Some(rect) = rect else {
            return;
        };
        let size = rect.size();
        if size.x < 480.0 || size.y < 360.0 {
            return;
        }

        if let Some(monitor) = monitor {
            let fills_screen = size.x >= monitor.x * 0.98 && size.y >= monitor.y * 0.94;
            if fills_screen {
                return;
            }
        }

        self.last_window_size = Some([size.x, size.y]);
    }

    fn center_window(&mut self, ctx: &Context) {
        if self.centered {
            return;
        }

        let (outer, monitor) = ctx.input(|i| {
            let viewport = i.viewport();
            (viewport.outer_rect, viewport.monitor_size)
        });

        let (Some(outer), Some(monitor)) = (outer, monitor) else {
            return;
        };

        if outer.width() <= 0.0 || outer.height() <= 0.0 {
            return;
        }

        let position = ((monitor - outer.size()) * 0.5).max(egui::Vec2::ZERO);
        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(
            position.x, position.y,
        )));
        self.centered = true;
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let colors = theme::chat_colors(ui.ctx().theme());
        let active_meta = self.store.conversations.active().and_then(|c| {
            if c.messages.is_empty() && (c.title.trim().is_empty() || c.title == "Nova conversa") {
                None
            } else {
                let title = if c.title.trim().is_empty() {
                    "Nova conversa".to_string()
                } else {
                    c.title.clone()
                };
                Some((c.id.clone(), title))
            }
        });

        egui::Panel::top("top_bar")
            .frame(
                egui::Frame::new()
                    .fill(colors.background)
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let toggle_tooltip = if self.sidebar_collapsed {
                        "Mostrar barra lateral (Ctrl+B)"
                    } else {
                        "Ocultar barra lateral (Ctrl+B)"
                    };

                    let (rect, resp) =
                        ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
                    if resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            egui::CornerRadius::same(6),
                            colors.border.gamma_multiply(0.5),
                        );
                    }
                    let icon_color = if resp.hovered() {
                        colors.assistant_text
                    } else {
                        colors.muted
                    };
                    let icon_rect = rect.shrink2(egui::vec2(6.5, 7.5));
                    ui.painter().rect_stroke(
                        icon_rect,
                        egui::CornerRadius::same(2),
                        egui::Stroke::new(1.3, icon_color),
                        egui::StrokeKind::Inside,
                    );
                    let split_x = icon_rect.left() + 5.0;
                    ui.painter().vline(
                        split_x,
                        icon_rect.y_range(),
                        egui::Stroke::new(1.3, icon_color),
                    );
                    if resp.on_hover_text(toggle_tooltip).clicked() {
                        self.toggle_sidebar();
                    }

                    if self.sidebar_collapsed {
                        let (plus_rect, plus_resp) =
                            ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
                        if plus_resp.hovered() {
                            ui.painter().rect_filled(
                                plus_rect,
                                egui::CornerRadius::same(6),
                                colors.border.gamma_multiply(0.5),
                            );
                        }
                        let plus_color = if plus_resp.hovered() {
                            colors.assistant_text
                        } else {
                            colors.muted
                        };
                        let c = plus_rect.center();
                        let s = 5.5;
                        ui.painter().hline(
                            (c.x - s)..=(c.x + s),
                            c.y,
                            egui::Stroke::new(1.4, plus_color),
                        );
                        ui.painter().vline(
                            c.x,
                            (c.y - s)..=(c.y + s),
                            egui::Stroke::new(1.4, plus_color),
                        );
                        if plus_resp
                            .on_hover_text("Nova conversa (Ctrl+N)")
                            .clicked()
                        {
                            self.new_conversation();
                        }
                    }

                    if let Some((active_id, active_title)) = active_meta {
                        ui.add_space(4.0);
                        let display_title = truncate_chars(&active_title, 48);
                        let title_btn = egui::Button::new(
                            egui::RichText::new(display_title)
                                .size(13.5)
                                .color(colors.muted),
                        )
                        .frame(false);
                        if ui
                            .add(title_btn)
                            .on_hover_text("Clique para renomear")
                            .clicked()
                        {
                            self.rename = Some((active_id, active_title));
                            self.rename_focus = true;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.sidebar_collapsed {
                            let (cfg_rect, cfg_resp) = ui.allocate_exact_size(
                                egui::vec2(28.0, 28.0),
                                egui::Sense::click(),
                            );
                            if cfg_resp.hovered() {
                                ui.painter().rect_filled(
                                    cfg_rect,
                                    egui::CornerRadius::same(6),
                                    colors.border.gamma_multiply(0.5),
                                );
                            }
                            let cfg_color = if cfg_resp.hovered() {
                                colors.assistant_text
                            } else {
                                colors.muted
                            };
                            let c = cfg_rect.center();
                            ui.painter()
                                .circle_stroke(c, 5.5, egui::Stroke::new(1.3, cfg_color));
                            ui.painter().circle_filled(c, 1.8, cfg_color);
                            if cfg_resp
                                .on_hover_text("Configurações (Ctrl+,)")
                                .clicked()
                            {
                                self.open_settings();
                            }
                            ui.add_space(4.0);
                        }

                        let models = self.model_list();
                        let mut selected = self.store.config.model.clone();
                        let mut fetch_from_combo = false;
                        let models_pending = self.models_pending;
                        egui::ComboBox::from_id_salt("model_combo")
                            .width(220.0)
                            .height(420.0)
                            .selected_text(
                                egui::RichText::new(selected.clone())
                                    .size(12.5)
                                    .color(colors.muted),
                            )
                            .show_ui(ui, |ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                ui.spacing_mut().interact_size.y = 24.0;
                                for model in &models {
                                    ui.selectable_value(&mut selected, model.clone(), model);
                                }
                                ui.separator();
                                let label = if models_pending {
                                    "Carregando modelos..."
                                } else {
                                    "Carregar todos os modelos"
                                };
                                if ui.add_enabled(!models_pending, egui::Button::new(label)).clicked() {
                                    fetch_from_combo = true;
                                }
                            });
                        if selected != self.store.config.model {
                            self.store.config.model = selected.clone();
                            self.settings_model = selected;
                            self.store.save();
                        }
                        if fetch_from_combo {
                            let ctx_clone = ui.ctx().clone();
                            self.start_fetch_models(&ctx_clone);
                        }
                    });
                });
            });
    }
}

impl eframe::App for NeoChat {
    fn logic(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.track_window_size(ctx);
        self.center_window(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if self.applied_theme != Some(self.store.config.theme) {
            theme::apply(&ctx, self.store.config.theme);
            theme::sync_title_bar(&ctx, self.store.config.theme);
            self.applied_theme = Some(self.store.config.theme);
        }

        let font_scale = self.store.config.font_scale;
        if self.applied_font_scale != Some(font_scale) {
            ctx.set_zoom_factor(font_scale);
            self.applied_font_scale = Some(font_scale);
        }

        self.handle_shortcuts(&ctx);
        self.poll_events();
        self.poll_test();
        self.poll_models();
        self.poll_login();

        sidebar::sidebar(self, ui);
        self.top_bar(ui);
        composer::composer(self, ui);
        chat::chat_area(self, ui);
        settings::settings_window(self, &ctx);
        sidebar::rename_window(self, &ctx);
        sidebar::delete_window(self, &ctx);
    }

    fn on_exit(&mut self) {
        if let Some(size) = self.last_window_size {
            self.store.config.window_size = Some(size);
        }
        self.store.save();
    }
}

pub(super) fn date_group(timestamp: i64) -> &'static str {
    use chrono::{Duration, Local, TimeZone};
    let Some(dt) = Local.timestamp_opt(timestamp, 0).single() else {
        return "Anteriores";
    };
    let today = Local::now().date_naive();
    let day = dt.date_naive();
    if day == today {
        "Hoje"
    } else if day == today - Duration::days(1) {
        "Ontem"
    } else {
        "Anteriores"
    }
}

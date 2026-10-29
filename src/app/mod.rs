mod chat;
mod composer;
mod settings;
mod sidebar;

use crate::mistral::{Message, MistralClient};
use crate::storage::{Store, StoredMessage, ThemeChoice};
use crate::{secrets, theme};
use eframe::egui;
use egui::Context;
use egui_commonmark::CommonMarkCache;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;

pub const SYSTEM_PROMPT: &str =
    "Você é o NeoChat, um assistente prestativo, claro e conciso. Responda no idioma do usuário.";

pub const SUGGESTIONS: &[&str] = &[
    "Explique um conceito de forma simples",
    "Escreva um e-mail profissional",
    "Resuma este texto em tópicos",
    "Ajude com um trecho de código",
];

pub enum Event {
    Token(String),
    Done,
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
    pub(super) error: Option<String>,
    pub(super) status: Option<String>,
    pub(super) settings_open: bool,
    pub(super) settings_key: String,
    pub(super) settings_model: String,
    pub(super) settings_theme: ThemeChoice,
    pub(super) test_rx: Option<Receiver<Result<String, String>>>,
    pub(super) test_pending: bool,
    pub(super) test_status: Option<(bool, String)>,
    pub(super) rename: Option<(String, String)>,
    pub(super) rename_focus: bool,
    pub(super) delete: Option<String>,
    pub(super) markdown_cache: CommonMarkCache,
    applied_theme: Option<ThemeChoice>,
    last_window_size: Option<[f32; 2]>,
    centered: bool,
    pub(super) follow_output: bool,
    pub(super) jump_to_bottom: bool,
    pub(super) focus_composer: bool,
}

impl NeoChat {
    pub fn new(cc: &eframe::CreationContext<'_>, store: Store) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx, store.config.theme);

        let api_key = std::env::var("MISTRAL_API_KEY")
            .ok()
            .map(|key| key.trim().to_string())
            .filter(|key| !key.is_empty())
            .or_else(secrets::load_api_key)
            .unwrap_or_default();

        let settings_model = store.config.model.clone();
        let settings_theme = store.config.theme;

        Self {
            rt: tokio::runtime::Runtime::new().expect("falha ao criar o runtime tokio"),
            store,
            api_key,
            input: String::new(),
            partial: String::new(),
            rx: None,
            cancel: None,
            error: None,
            status: None,
            settings_open: false,
            settings_key: String::new(),
            settings_model,
            settings_theme,
            test_rx: None,
            test_pending: false,
            test_status: None,
            rename: None,
            rename_focus: false,
            delete: None,
            markdown_cache: CommonMarkCache::default(),
            applied_theme: None,
            last_window_size: None,
            centered: false,
            follow_output: true,
            jump_to_bottom: false,
            focus_composer: false,
        }
    }

    pub fn busy(&self) -> bool {
        self.rx.is_some()
    }

    fn model_list(&self) -> Vec<String> {
        let mut models: Vec<String> = crate::storage::MODELS
            .iter()
            .map(|m| m.to_string())
            .collect();
        if !models.contains(&self.store.config.model) {
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
        self.test_status = None;
        self.settings_open = true;
    }

    pub(super) fn new_conversation(&mut self) {
        if self.busy() {
            self.stop();
        }
        self.store.conversations.new_conversation();
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
        self.store.conversations.active = Some(id.to_string());
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

    fn push_assistant(&mut self, content: String, interrupted: bool) {
        if let Some(conversation) = self.store.conversations.active_mut() {
            let mut message = StoredMessage::new("assistant", content);
            message.interrupted = interrupted;
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
        if self.api_key.trim().is_empty() {
            self.error = Some("Informe a chave da API da Mistral nas configurações.".to_string());
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

        let client = MistralClient::new(
            self.api_key.trim().to_string(),
            self.store.config.model.clone(),
        );
        let (tx, rx) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_task = Arc::clone(&cancel);
        let ctx_task = ctx.clone();

        self.rx = Some(rx);
        self.cancel = Some(cancel);
        self.partial.clear();

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
                Ok(_) => {
                    tx.send(Event::Done).ok();
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
            self.push_assistant(reply, true);
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
        if self.api_key.trim().is_empty() {
            self.error = Some("Informe a chave da API da Mistral nas configurações.".to_string());
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
                    self.follow_output = true;
                }
                Ok(Event::Done) => {
                    let reply = std::mem::take(&mut self.partial);
                    if !reply.trim().is_empty() {
                        self.push_assistant(reply, false);
                    }
                    self.cancel = None;
                    self.rx = None;
                    return;
                }
                Ok(Event::Error(err)) => {
                    self.error = Some(err);
                    let reply = std::mem::take(&mut self.partial);
                    if !reply.trim().is_empty() {
                        self.push_assistant(reply, true);
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
                        self.push_assistant(reply, true);
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

    pub(super) fn apply_settings(&mut self) {
        self.api_key = self.settings_key.trim().to_string();
        if let Err(err) = secrets::save_api_key(&self.api_key) {
            self.error = Some(format!("Falha ao salvar a chave: {err:#}"));
        }
        self.store.config.model = self.settings_model.clone();
        self.store.config.theme = self.settings_theme;
        self.store.save();
        self.settings_open = false;
        self.status = Some("Configurações salvas.".to_string());
        self.error = None;
    }

    fn handle_shortcuts(&mut self, ctx: &Context) {
        let (new_chat, settings, escape) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::N) && i.modifiers.command,
                i.key_pressed(egui::Key::Comma) && i.modifiers.command,
                i.key_pressed(egui::Key::Escape),
            )
        });
        if new_chat {
            self.new_conversation();
        }
        if settings {
            self.open_settings();
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
        egui::Panel::top("top_bar")
            .frame(
                egui::Frame::new()
                    .fill(colors.surface)
                    .inner_margin(egui::Margin::symmetric(14, 8)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("NeoChat")
                            .size(18.0)
                            .strong()
                            .color(colors.accent),
                    );
                    ui.add_space(2.0);
                    ui.label(egui::RichText::new("Mistral").small().color(colors.muted));

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new("Configurações").size(14.0),
                            ))
                            .clicked()
                        {
                            self.open_settings();
                        }

                        let models = self.model_list();
                        let mut selected = self.store.config.model.clone();
                        egui::ComboBox::from_id_salt("model_combo")
                            .width(190.0)
                            .selected_text(egui::RichText::new(selected.clone()).size(14.0))
                            .show_ui(ui, |ui| {
                                for model in &models {
                                    ui.selectable_value(&mut selected, model.clone(), model);
                                }
                            });
                        if selected != self.store.config.model {
                            self.store.config.model = selected;
                            self.store.save();
                        }
                    });
                });

                let clip = ui.clip_rect();
                ui.painter().hline(
                    clip.x_range(),
                    clip.bottom() - 0.5,
                    egui::Stroke::new(1.0, colors.border),
                );
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

        self.handle_shortcuts(&ctx);
        self.poll_events();
        self.poll_test();

        self.top_bar(ui);
        sidebar::sidebar(self, ui);
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

pub(super) fn format_time(timestamp: i64) -> String {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.format("%H:%M").to_string())
        .unwrap_or_default()
}

pub(super) fn format_day(timestamp: i64) -> String {
    use chrono::{Local, TimeZone};
    Local
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| {
            let today = Local::now().date_naive();
            if dt.date_naive() == today {
                dt.format("%H:%M").to_string()
            } else {
                dt.format("%d/%m").to_string()
            }
        })
        .unwrap_or_default()
}

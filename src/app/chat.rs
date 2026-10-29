use super::{format_time, NeoChat, SUGGESTIONS};
use crate::storage::StoredMessage;
use crate::theme::{self, ChatColors};
use egui::{Align, Color32, Frame, Label, Layout, Margin, RichText, Stroke, Ui};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

enum ChatAction {
    Copy(String),
    Regenerate,
    Suggestion(String),
}

pub(super) fn chat_area(app: &mut NeoChat, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let colors = theme::chat_colors(ctx.theme());
    let messages: Vec<StoredMessage> = app
        .store
        .conversations
        .active()
        .map(|conversation| conversation.messages.clone())
        .unwrap_or_default();
    let busy = app.busy();
    let last_assistant = messages.iter().rposition(|m| m.role == "assistant");
    let mut action: Option<ChatAction> = None;
    let mut cache = std::mem::take(&mut app.markdown_cache);
    let mut follow = app.follow_output;
    let jump = app.jump_to_bottom;

    if ui.input(|i| i.smooth_scroll_delta.y.abs() > 0.5) {
        follow = false;
    }

    let panel = egui::CentralPanel::default()
        .frame(
            Frame::new()
                .fill(colors.background)
                .inner_margin(Margin::symmetric(18, 0)),
        )
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("chat_scroll")
                .auto_shrink([false, false])
                .stick_to_bottom(follow)
                .show(ui, |ui| {
                    if messages.is_empty() && !busy {
                        empty_state(ui, &colors, &mut action);
                    } else {
                        for (index, message) in messages.iter().enumerate() {
                            if message.is_user() {
                                user_bubble(ui, &colors, message);
                            } else {
                                let is_last = Some(index) == last_assistant;
                                assistant_bubble(
                                    ui,
                                    &colors,
                                    &mut cache,
                                    message,
                                    is_last,
                                    busy,
                                    &mut action,
                                );
                            }
                            ui.add_space(8.0);
                        }
                    }

                    if busy {
                        let text = if app.partial.is_empty() {
                            ""
                        } else {
                            app.partial.as_str()
                        };
                        streaming_bubble(ui, &colors, &mut cache, text);
                    }

                    ui.add_space(10.0);

                    if jump {
                        ui.scroll_to_cursor(Some(Align::BOTTOM));
                    }
                })
        });

    app.markdown_cache = cache;

    let scroll = panel.inner;
    let max_offset = (scroll.content_size.y - scroll.inner_rect.height()).max(0.0);
    let at_bottom = scroll.state.clamped_offset().y >= max_offset - 12.0;
    app.follow_output = at_bottom || jump;
    app.jump_to_bottom = false;

    if !at_bottom {
        let rect = scroll.inner_rect;
        let pos = egui::pos2(rect.center().x - 62.0, rect.bottom() - 44.0);
        egui::Area::new(egui::Id::unique("jump_to_bottom"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(&ctx, |ui| {
                if ui
                    .add(egui::Button::new("Ir para o fim").fill(colors.surface))
                    .clicked()
                {
                    app.follow_output = true;
                    app.jump_to_bottom = true;
                }
            });
    }

    match action {
        Some(ChatAction::Copy(text)) => {
            ctx.copy_text(text);
            app.status = Some("Mensagem copiada.".to_string());
        }
        Some(ChatAction::Regenerate) => app.regenerate(&ctx),
        Some(ChatAction::Suggestion(text)) => {
            app.input = text;
            app.focus_composer = true;
        }
        None => {}
    }
}

fn user_bubble(ui: &mut Ui, colors: &ChatColors, message: &StoredMessage) {
    ui.with_layout(Layout::top_down(Align::Max), |ui| {
        let max = ui.available_width() * 0.86;
        ui.set_max_width(max);
        Frame::new()
            .fill(colors.user_bubble)
            .corner_radius(12)
            .inner_margin(Margin::symmetric(12, 9))
            .show(ui, |ui| {
                ui.add(
                    Label::new(
                        RichText::new(&message.content)
                            .color(colors.user_text)
                            .size(15.0),
                    )
                    .selectable(true)
                    .wrap(),
                );
                ui.add(
                    Label::new(
                        RichText::new(format_time(message.timestamp))
                            .small()
                            .color(colors.user_text.gamma_multiply(0.72)),
                    )
                    .selectable(false),
                );
            });
    });
}

fn assistant_bubble(
    ui: &mut Ui,
    colors: &ChatColors,
    cache: &mut CommonMarkCache,
    message: &StoredMessage,
    is_last: bool,
    busy: bool,
    action: &mut Option<ChatAction>,
) {
    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        let max = ui.available_width() * 0.88;
        ui.set_max_width(max);
        let frame = Frame::new()
            .fill(colors.assistant_bubble)
            .stroke(Stroke::new(1.0, colors.assistant_border))
            .corner_radius(12)
            .inner_margin(Margin::symmetric(12, 9))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("NeoChat")
                            .small()
                            .strong()
                            .color(colors.accent),
                    );
                    ui.label(
                        RichText::new(format_time(message.timestamp))
                            .small()
                            .color(colors.muted),
                    );
                    if message.interrupted {
                        ui.label(
                            RichText::new("interrompida")
                                .small()
                                .italics()
                                .color(colors.muted),
                        );
                    }
                });
                ui.add_space(2.0);
                CommonMarkViewer::new().show(ui, cache, &message.content);
                if is_last && !busy {
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.small_button("Copiar").clicked() {
                            *action = Some(ChatAction::Copy(message.content.clone()));
                        }
                        if ui.small_button("Regenerar").clicked() {
                            *action = Some(ChatAction::Regenerate);
                        }
                    });
                }
            });

        frame.response.context_menu(|ui| {
            if ui.button("Copiar").clicked() {
                *action = Some(ChatAction::Copy(message.content.clone()));
                ui.close();
            }
            if is_last && ui.button("Regenerar").clicked() {
                *action = Some(ChatAction::Regenerate);
                ui.close();
            }
        });
    });
}

fn streaming_bubble(ui: &mut Ui, colors: &ChatColors, cache: &mut CommonMarkCache, text: &str) {
    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        let max = ui.available_width() * 0.88;
        ui.set_max_width(max);
        Frame::new()
            .fill(colors.assistant_bubble)
            .stroke(Stroke::new(1.0, colors.assistant_border))
            .corner_radius(12)
            .inner_margin(Margin::symmetric(12, 9))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("NeoChat")
                            .small()
                            .strong()
                            .color(colors.accent),
                    );
                    ui.spinner();
                });
                ui.add_space(2.0);
                if text.is_empty() {
                    ui.label(RichText::new("Pensando...").italics().color(colors.muted));
                } else {
                    CommonMarkViewer::new().show(ui, cache, text);
                }
            });
    });
}

fn empty_state(ui: &mut Ui, colors: &ChatColors, action: &mut Option<ChatAction>) {
    let top = ((ui.available_height() - 260.0) * 0.5).max(24.0);
    ui.add_space(top);
    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("Como posso ajudar hoje?")
                .size(27.0)
                .strong()
                .color(colors.assistant_text),
        );
        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "Converse com um modelo da Mistral. Suas conversas ficam salvas neste computador.",
            )
            .color(colors.muted),
        );
        ui.add_space(22.0);
        let chips_width = ui.available_width().min(620.0);
        ui.allocate_ui_with_layout(
            egui::vec2(chips_width, 120.0),
            Layout::top_down(Align::Center),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    for suggestion in SUGGESTIONS {
                        let button = egui::Button::new(
                            RichText::new(*suggestion)
                                .color(colors.assistant_text)
                                .size(14.0),
                        )
                        .fill(colors.surface)
                        .stroke(Stroke::new(1.0, colors.border))
                        .corner_radius(8);
                        if ui.add(button).clicked() {
                            *action = Some(ChatAction::Suggestion(suggestion.to_string()));
                        }
                    }
                });
            },
        );
        ui.add_space(12.0);
        ui.label(
            RichText::new("Ctrl+N nova conversa · Ctrl+, configurações")
                .small()
                .color(Color32::from_rgb(0x80, 0x80, 0x8C)),
        );
    });
}

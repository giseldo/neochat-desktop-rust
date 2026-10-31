use super::NeoChat;
use crate::storage::StoredMessage;
use crate::theme::{self, ChatColors, MAX_CHAT_WIDTH};
use egui::{
    Align, Align2, Button, Color32, CornerRadius, Frame, Label, Layout, Margin, RichText, Sense,
    Stroke, StrokeKind, Ui,
};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use std::time::{Duration, Instant};

enum ChatAction {
    Copy(usize, String),
    Regenerate,
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

    let copied_index = app.copied_feedback.and_then(|(idx, instant)| {
        if instant.elapsed() < Duration::from_secs(2) {
            ctx.request_repaint_after(Duration::from_millis(250));
            Some(idx)
        } else {
            None
        }
    });
    if copied_index.is_none() {
        app.copied_feedback = None;
    }

    if ui.input(|i| i.smooth_scroll_delta.y.abs() > 0.5) {
        follow = false;
    }

    let panel = egui::CentralPanel::default()
        .frame(
            Frame::new()
                .fill(colors.background)
                .inner_margin(Margin::same(0)),
        )
        .show(ui, |ui| {
            if messages.is_empty() && !busy {
                let rect = ui.available_rect_before_wrap();
                ui.painter().text(
                    egui::pos2(rect.center().x, rect.center().y - 16.0),
                    Align2::CENTER_CENTER,
                    "Como posso ajudar?",
                    egui::FontId::proportional(22.0),
                    colors.assistant_text.gamma_multiply(0.85),
                );
            }

            egui::ScrollArea::vertical()
                .id_salt("chat_scroll")
                .auto_shrink([false, false])
                .stick_to_bottom(follow)
                .show(ui, |ui| {
                    let avail_w = ui.available_width();
                    let col_w = (avail_w - 48.0).clamp(260.0, MAX_CHAT_WIDTH);
                    let side_pad = ((avail_w - col_w) * 0.5).max(24.0);

                    ui.add_space(24.0);
                    ui.horizontal(|ui| {
                        ui.add_space(side_pad);
                        ui.vertical(|ui| {
                            ui.set_width(col_w);
                            ui.set_max_width(col_w);

                            for (index, message) in messages.iter().enumerate() {
                                if message.is_user() {
                                    user_bubble(ui, &colors, message);
                                } else {
                                    let is_last = Some(index) == last_assistant;
                                    let is_copied = copied_index == Some(index);
                                    assistant_bubble(
                                        ui,
                                        &colors,
                                        &mut cache,
                                        index,
                                        message,
                                        is_last,
                                        is_copied,
                                        busy,
                                        &mut action,
                                    );
                                }
                                ui.add_space(22.0);
                            }

                            if busy {
                                let text = if app.partial.is_empty() {
                                    ""
                                } else {
                                    app.partial.as_str()
                                };
                                streaming_bubble(ui, &colors, &mut cache, text);
                            }

                            ui.add_space(28.0);

                            if jump {
                                ui.scroll_to_cursor(Some(Align::BOTTOM));
                            }
                        });
                    });
                })
        });

    app.markdown_cache = cache;

    let scroll = panel.inner;
    let max_offset = (scroll.content_size.y - scroll.inner_rect.height()).max(0.0);
    let at_bottom = scroll.state.clamped_offset().y >= max_offset - 12.0;
    app.follow_output = at_bottom || jump;
    app.jump_to_bottom = false;

    if !at_bottom && !messages.is_empty() {
        let rect = scroll.inner_rect;
        let pos = egui::pos2(rect.center().x - 18.0, rect.bottom() - 44.0);
        egui::Area::new(egui::Id::unique("jump_to_bottom"))
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(&ctx, |ui| {
                let btn = Button::new(
                    RichText::new("↓")
                        .size(14.0)
                        .strong()
                        .color(colors.assistant_text),
                )
                .fill(colors.surface)
                .stroke(Stroke::new(1.0, colors.border))
                .corner_radius(CornerRadius::same(18))
                .min_size(egui::vec2(34.0, 34.0));
                if ui
                    .add(btn)
                    .on_hover_text("Ir para o fim da conversa")
                    .clicked()
                {
                    app.follow_output = true;
                    app.jump_to_bottom = true;
                }
            });
    }

    match action {
        Some(ChatAction::Copy(index, text)) => {
            ctx.copy_text(text);
            app.copied_feedback = Some((index, Instant::now()));
        }
        Some(ChatAction::Regenerate) => app.regenerate(&ctx),
        None => {}
    }
}

fn user_bubble(ui: &mut Ui, colors: &ChatColors, message: &StoredMessage) {
    ui.with_layout(Layout::top_down(Align::Max), |ui| {
        let max = (ui.available_width() * 0.76).max(220.0);
        ui.set_max_width(max);
        Frame::new()
            .fill(colors.user_bubble)
            .corner_radius(CornerRadius::same(18))
            .inner_margin(Margin::symmetric(16, 10))
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
            });
    });
}

#[allow(clippy::too_many_arguments)]
fn assistant_bubble(
    ui: &mut Ui,
    colors: &ChatColors,
    cache: &mut CommonMarkCache,
    index: usize,
    message: &StoredMessage,
    is_last: bool,
    is_copied: bool,
    busy: bool,
    action: &mut Option<ChatAction>,
) {
    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        ui.set_max_width(ui.available_width());
        let frame = Frame::new()
            .fill(Color32::TRANSPARENT)
            .inner_margin(Margin::symmetric(2, 2))
            .show(ui, |ui| {
                CommonMarkViewer::new().show(ui, cache, &message.content);

                if message.interrupted {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("Resposta interrompida")
                            .size(11.5)
                            .italics()
                            .color(colors.muted),
                    );
                }

                if !busy {
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 4.0;

                        if ghost_action_button(
                            ui,
                            colors,
                            if is_copied { "Copiado" } else { "Copiar" },
                            is_copied,
                            ActionIcon::Copy { done: is_copied },
                        )
                        .on_hover_text("Copiar resposta")
                        .clicked()
                        {
                            *action = Some(ChatAction::Copy(index, message.content.clone()));
                        }

                        if is_last
                            && ghost_action_button(
                                ui,
                                colors,
                                "Regenerar",
                                false,
                                ActionIcon::Regenerate,
                            )
                            .on_hover_text("Gerar uma nova resposta")
                            .clicked()
                        {
                            *action = Some(ChatAction::Regenerate);
                        }
                    });
                }
            });

        frame.response.context_menu(|ui| {
            if ui.button("Copiar").clicked() {
                *action = Some(ChatAction::Copy(index, message.content.clone()));
                ui.close();
            }
            if is_last && ui.button("Regenerar").clicked() {
                *action = Some(ChatAction::Regenerate);
                ui.close();
            }
        });
    });
}

enum ActionIcon {
    Copy { done: bool },
    Regenerate,
}

fn ghost_action_button(
    ui: &mut Ui,
    colors: &ChatColors,
    label: &str,
    active: bool,
    icon: ActionIcon,
) -> egui::Response {
    let font_id = egui::FontId::proportional(12.0);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), font_id.clone(), colors.muted);
    let width = galley.size().x + 28.0;
    let height = 24.0;

    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, height), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(6),
            colors.border.gamma_multiply(0.55),
        );
    }

    let fg = if active {
        colors.accent
    } else if resp.hovered() {
        colors.assistant_text
    } else {
        colors.muted
    };

    let ic = egui::pos2(rect.left() + 11.0, rect.center().y);
    match icon {
        ActionIcon::Copy { done: true } => {
            let stroke = Stroke::new(1.5, fg);
            ui.painter().line_segment(
                [egui::pos2(ic.x - 4.0, ic.y), egui::pos2(ic.x - 1.0, ic.y + 3.0)],
                stroke,
            );
            ui.painter().line_segment(
                [egui::pos2(ic.x - 1.0, ic.y + 3.0), egui::pos2(ic.x + 4.5, ic.y - 3.0)],
                stroke,
            );
        }
        ActionIcon::Copy { done: false } => {
            let stroke = Stroke::new(1.2, fg);
            let back = egui::Rect::from_min_size(
                egui::pos2(ic.x - 4.0, ic.y - 4.5),
                egui::vec2(6.5, 6.5),
            );
            let front = egui::Rect::from_min_size(
                egui::pos2(ic.x - 1.5, ic.y - 2.0),
                egui::vec2(6.5, 6.5),
            );
            ui.painter()
                .rect_stroke(back, CornerRadius::same(1), stroke, StrokeKind::Inside);
            ui.painter()
                .rect_filled(front, CornerRadius::same(1), colors.background);
            ui.painter()
                .rect_stroke(front, CornerRadius::same(1), stroke, StrokeKind::Inside);
        }
        ActionIcon::Regenerate => {
            let stroke = Stroke::new(1.3, fg);
            ui.painter().circle_stroke(ic, 4.2, stroke);
            ui.painter().line_segment(
                [egui::pos2(ic.x + 2.5, ic.y - 4.2), egui::pos2(ic.x + 4.8, ic.y - 1.8)],
                stroke,
            );
        }
    }

    ui.painter().text(
        egui::pos2(rect.left() + 21.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        font_id,
        fg,
    );

    resp
}

fn streaming_bubble(ui: &mut Ui, colors: &ChatColors, cache: &mut CommonMarkCache, text: &str) {
    ui.with_layout(Layout::top_down(Align::Min), |ui| {
        ui.set_max_width(ui.available_width());
        Frame::new()
            .fill(Color32::TRANSPARENT)
            .inner_margin(Margin::symmetric(2, 2))
            .show(ui, |ui| {
                if text.is_empty() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new("Pensando...")
                                .size(13.5)
                                .italics()
                                .color(colors.muted),
                        );
                    });
                } else {
                    CommonMarkViewer::new().show(ui, cache, text);
                }
            });
    });
}

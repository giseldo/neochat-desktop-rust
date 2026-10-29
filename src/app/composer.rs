use super::NeoChat;
use crate::theme;
use egui::{Button, Color32, Frame, Label, Margin, Panel, RichText, TextEdit, Ui};

pub(super) fn composer(app: &mut NeoChat, ui: &mut Ui) {
    let ctx = ui.ctx().clone();
    let colors = theme::chat_colors(ctx.theme());

    Panel::bottom("composer")
        .resizable(false)
        .frame(
            Frame::new()
                .fill(colors.surface)
                .inner_margin(Margin::symmetric(14, 10)),
        )
        .show(ui, |ui| {
            if let Some(error) = app.error.clone() {
                ui.horizontal(|ui| {
                    ui.add(Label::new(
                        RichText::new(error)
                            .small()
                            .color(ui.visuals().error_fg_color),
                    ));
                    if ui.small_button("Fechar").clicked() {
                        app.error = None;
                    }
                });
            } else if let Some(status) = app.status.clone() {
                ui.horizontal(|ui| {
                    ui.add(Label::new(
                        RichText::new(status).small().color(colors.muted),
                    ));
                    if ui.small_button("Fechar").clicked() {
                        app.status = None;
                    }
                });
            }

            ui.horizontal(|ui| {
                let send_width = 108.0;
                let spacing = ui.spacing().item_spacing.x;
                let edit_width = (ui.available_width() - send_width - spacing).max(140.0);

                let edit = ui.add_sized(
                    [edit_width, 56.0],
                    TextEdit::multiline(&mut app.input)
                        .hint_text("Digite sua mensagem...")
                        .desired_rows(2)
                        .desired_width(f32::INFINITY),
                );

                if app.focus_composer {
                    edit.request_focus();
                    app.focus_composer = false;
                }

                let enter = edit.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
                if enter {
                    while app.input.ends_with('\n') {
                        app.input.pop();
                    }
                }

                let busy = app.busy();
                let (label, fill) = if busy {
                    ("Parar", Color32::from_rgb(0xB3, 0x26, 0x1A))
                } else {
                    ("Enviar", colors.user_bubble)
                };
                let button = Button::new(
                    RichText::new(label)
                        .color(Color32::WHITE)
                        .strong()
                        .size(15.0),
                )
                .fill(fill)
                .corner_radius(10)
                .min_size(egui::vec2(send_width, 56.0));

                let enabled = busy || !app.input.trim().is_empty();
                if ui.add_enabled(enabled, button).clicked() {
                    if busy {
                        app.stop();
                    } else {
                        app.send(&ctx);
                        edit.request_focus();
                    }
                } else if enter && !busy && !app.input.trim().is_empty() {
                    app.send(&ctx);
                    edit.request_focus();
                }
            });

            ui.add_space(4.0);
            ui.add(Label::new(
                RichText::new("Enter envia · Shift+Enter quebra linha · Esc interrompe")
                    .small()
                    .color(colors.muted),
            ));
        });
}

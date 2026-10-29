use super::{format_day, NeoChat};
use crate::theme::{self, ChatColors};
use egui::{
    Align, Align2, Button, Color32, CornerRadius, Frame, Label, Layout, Margin, Panel, Response,
    RichText, Sense, Stroke, StrokeKind, TextEdit, Ui, UiBuilder, Window,
};

enum RowAction {
    Select(String),
    Rename(String),
    Delete(String),
}

pub(super) fn sidebar(app: &mut NeoChat, ui: &mut Ui) {
    let colors = theme::chat_colors(ui.ctx().theme());

    Panel::left("sidebar")
        .resizable(true)
        .default_size(274.0)
        .min_size(226.0)
        .max_size(390.0)
        .frame(
            Frame::new()
                .fill(colors.surface)
                .inner_margin(Margin::symmetric(10, 10)),
        )
        .show(ui, |ui| {
            egui::CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(colors.surface)
                        .inner_margin(Margin::same(0)),
                )
                .show(ui, |ui| {
                    let new_button = Button::new(
                        RichText::new("+  Nova conversa")
                            .color(Color32::WHITE)
                            .strong(),
                    )
                    .fill(colors.user_bubble)
                    .corner_radius(8);

                    if ui
                        .add_sized([ui.available_width(), 36.0], new_button)
                        .clicked()
                    {
                        app.new_conversation();
                    }

                    ui.add_space(10.0);
                    ui.add(Label::new(
                        RichText::new("CONVERSAS").small().color(colors.muted),
                    ));
                    ui.add_space(4.0);

                    let active = app.store.conversations.active.clone();
                    let rows: Vec<(String, String, String, i64)> = app
                        .store
                        .conversations
                        .items
                        .iter()
                        .map(|conversation| {
                            (
                                conversation.id.clone(),
                                if conversation.title.trim().is_empty() {
                                    "Nova conversa".to_string()
                                } else {
                                    conversation.title.clone()
                                },
                                conversation.preview(),
                                conversation.updated_at,
                            )
                        })
                        .collect();

                    let mut action: Option<RowAction> = None;

egui::ScrollArea::vertical()
                            .id_salt("conversation_list")
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                let mut collapsed = false;
                                if ui.button("Toggle Collapse").clicked() {
                                    collapsed = !collapsed;
                                }
                                if !collapsed {
                            for (id, title, preview, updated) in &rows {
                                let selected = active.as_deref() == Some(id.as_str());
                                let response = row(ui, &colors, title, preview, *updated, selected);
                                if response.clicked() {
                                    action = Some(RowAction::Select(id.clone()));
                                }
                                response.context_menu(|ui| {
                                    if ui.button("Renomear").clicked() {
                                        action = Some(RowAction::Rename(id.clone()));
                                        ui.close();
                                    }
                                    if ui.button("Excluir").clicked() {
                                        action = Some(RowAction::Delete(id.clone()));
                                        ui.close();
                                    }
                                });
                                ui.add_space(2.0);
                            }
                        }
                    });

                    match action {
                        Some(RowAction::Select(id)) => app.select_conversation(&id),
                        Some(RowAction::Rename(id)) => {
                            let current = app
                                .store
                                .conversations
                                .items
                                .iter()
                                .find(|c| c.id == id)
                                .map(|c| c.title.clone())
                                .unwrap_or_default();
                            app.rename = Some((id, current));
                            app.rename_focus = true;
                        }
                        Some(RowAction::Delete(id)) => app.delete = Some(id),
                        None => {}
                    }
                });
        });
}

fn row(
    ui: &mut Ui,
    colors: &ChatColors,
    title: &str,
    preview: &str,
    updated: i64,
    selected: bool,
) -> Response {
    let height = 56.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click());

    if ui.is_rect_visible(rect) {
        let radius = CornerRadius::same(8);
        let (hover, selected_bg) = if ui.visuals().dark_mode {
            (
                Color32::from_rgb(0x2A, 0x2A, 0x33),
                Color32::from_rgb(0x1E, 0x39, 0x52),
            )
        } else {
            (
                Color32::from_rgb(0xF0, 0xF0, 0xF5),
                Color32::from_rgb(0xDD, 0xEC, 0xFA),
            )
        };

        if selected {
            ui.painter().rect_filled(rect, radius, selected_bg);
            ui.painter().rect_stroke(
                rect,
                radius,
                Stroke::new(1.0, colors.accent.gamma_multiply(0.55)),
                StrokeKind::Inside,
            );
        } else if response.hovered() {
            ui.painter().rect_filled(rect, radius, hover);
        }

        let inner = rect.shrink2(egui::vec2(10.0, 6.0));
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::top_down(Align::Min)),
        );
        child.spacing_mut().item_spacing.y = 2.0;

        let title_color = if selected {
            colors.accent
        } else {
            ui.visuals().strong_text_color()
        };
        child.add(
            Label::new(RichText::new(title).strong().size(14.0).color(title_color))
                .truncate()
                .selectable(false),
        );
        child.add(
            Label::new(
                RichText::new(format!("{} · {}", format_day(updated), preview))
                    .small()
                    .color(colors.muted),
            )
            .truncate()
            .selectable(false),
        );
    }

    response
}

pub(super) fn rename_window(app: &mut NeoChat, ctx: &egui::Context) {
    let Some((id, mut buffer)) = app.rename.clone() else {
        return;
    };

    let mut open = true;
    let mut save = false;
    let mut cancel = false;
    let mut focus = app.rename_focus;

    Window::new("Renomear conversa")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label("Novo título:");
            let edit = ui.add(
                TextEdit::singleline(&mut buffer)
                    .desired_width(360.0)
                    .hint_text("Título da conversa"),
            );
            if focus {
                edit.request_focus();
                focus = false;
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Salvar").clicked() {
                    save = true;
                }
                if ui.button("Cancelar").clicked() {
                    cancel = true;
                }
            });
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                save = true;
            }
        });

    app.rename_focus = focus;

    if save && !buffer.trim().is_empty() {
        if let Some(conversation) = app
            .store
            .conversations
            .items
            .iter_mut()
            .find(|c| c.id == id)
        {
            conversation.title = buffer.trim().to_string();
            conversation.touch();
        }
        app.store.save();
        app.rename = None;
    } else if cancel || !open {
        app.rename = None;
    } else {
        app.rename = Some((id, buffer));
    }
}

pub(super) fn delete_window(app: &mut NeoChat, ctx: &egui::Context) {
    let Some(id) = app.delete.clone() else {
        return;
    };

    let title = app
        .store
        .conversations
        .items
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.title.clone())
        .unwrap_or_else(|| "esta conversa".to_string());

    let mut open = true;
    let mut confirm = false;
    let mut cancel = false;

    Window::new("Excluir conversa")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(format!("Excluir \"{title}\"?"));
            ui.label(RichText::new("Essa ação não pode ser desfeita.").weak());
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        Button::new(RichText::new("Excluir").color(Color32::WHITE))
                            .fill(Color32::from_rgb(0xB3, 0x26, 0x1A)),
                    )
                    .clicked()
                {
                    confirm = true;
                }
                if ui.button("Cancelar").clicked() {
                    cancel = true;
                }
            });
        });

    if confirm {
        app.delete_conversation(&id);
        app.delete = None;
    } else if cancel || !open {
        app.delete = None;
    }
}

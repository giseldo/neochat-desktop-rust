use super::{date_group, NeoChat};
use crate::theme::{self, ChatColors};
use egui::{
    Align, Align2, Button, Color32, CornerRadius, Frame, Label, Layout, Margin, Panel, RichText,
    Sense, Stroke, TextEdit, Ui, UiBuilder, Window,
};

enum RowAction {
    Select(String),
    Rename(String),
    Delete(String),
}

pub(super) fn sidebar(app: &mut NeoChat, ui: &mut Ui) {
    if app.sidebar_collapsed {
        return;
    }

    let colors = theme::chat_colors(ui.ctx().theme());

    let panel_resp = Panel::left("sidebar")
        .resizable(true)
        .default_size(248.0)
        .min_size(208.0)
        .max_size(340.0)
        .frame(
            Frame::new()
                .fill(colors.sidebar_bg)
                .inner_margin(Margin::symmetric(10, 12)),
        )
        .show(ui, |ui| {
            Panel::bottom("sidebar_footer")
                .resizable(false)
                .frame(
                    Frame::new()
                        .fill(colors.sidebar_bg)
                        .inner_margin(Margin::symmetric(0, 4)),
                )
                .show(ui, |ui| {
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 32.0),
                        Sense::click(),
                    );
                    if resp.hovered() {
                        ui.painter().rect_filled(
                            rect,
                            CornerRadius::same(7),
                            colors.border.gamma_multiply(0.55),
                        );
                    }
                    let fg = if resp.hovered() {
                        colors.assistant_text
                    } else {
                        colors.muted
                    };
                    let icon_center = egui::pos2(rect.left() + 16.0, rect.center().y);
                    ui.painter()
                        .circle_stroke(icon_center, 5.0, Stroke::new(1.3, fg));
                    ui.painter().circle_filled(icon_center, 1.6, fg);

                    ui.painter().text(
                        egui::pos2(rect.left() + 30.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        "Configurações",
                        egui::FontId::proportional(13.0),
                        fg,
                    );
                    if resp
                        .on_hover_text("Abrir configurações (Ctrl+,)")
                        .clicked()
                    {
                        app.open_settings();
                    }
                });

            egui::CentralPanel::default()
                .frame(
                    Frame::new()
                        .fill(colors.sidebar_bg)
                        .inner_margin(Margin::same(0)),
                )
                .show(ui, |ui| {
                    let (btn_rect, btn_resp) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 34.0),
                        Sense::click(),
                    );
                    let btn_bg = if btn_resp.hovered() {
                        colors.border.gamma_multiply(0.45)
                    } else {
                        colors.surface
                    };
                    ui.painter()
                        .rect_filled(btn_rect, CornerRadius::same(8), btn_bg);
                    ui.painter().rect_stroke(
                        btn_rect,
                        CornerRadius::same(8),
                        Stroke::new(1.0, colors.border),
                        egui::StrokeKind::Inside,
                    );

                    let plus_c = egui::pos2(btn_rect.left() + 16.0, btn_rect.center().y);
                    let s = 4.5;
                    ui.painter().hline(
                        (plus_c.x - s)..=(plus_c.x + s),
                        plus_c.y,
                        Stroke::new(1.4, colors.assistant_text),
                    );
                    ui.painter().vline(
                        plus_c.x,
                        (plus_c.y - s)..=(plus_c.y + s),
                        Stroke::new(1.4, colors.assistant_text),
                    );
                    ui.painter().text(
                        egui::pos2(btn_rect.left() + 30.0, btn_rect.center().y),
                        Align2::LEFT_CENTER,
                        "Nova conversa",
                        egui::FontId::proportional(13.5),
                        colors.assistant_text,
                    );

                    if btn_resp
                        .on_hover_text("Criar uma nova conversa (Ctrl+N)")
                        .clicked()
                    {
                        app.new_conversation();
                    }

                    ui.add_space(12.0);

                    let active = app.store.conversations.active.clone();
                    let mut rows: Vec<(String, String, i64)> = app
                        .store
                        .conversations
                        .items
                        .iter()
                        .filter(|c| !c.messages.is_empty() || app.store.conversations.items.len() == 1)
                        .map(|conversation| {
                            (
                                conversation.id.clone(),
                                if conversation.title.trim().is_empty() {
                                    "Nova conversa".to_string()
                                } else {
                                    conversation.title.clone()
                                },
                                conversation.updated_at,
                            )
                        })
                        .collect();
                    rows.sort_by_key(|a| std::cmp::Reverse(a.2));

                    let mut action: Option<RowAction> = None;

                    egui::ScrollArea::vertical()
                        .id_salt("conversation_list")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let mut current_group = "";
                            for (id, title, updated) in &rows {
                                let group = date_group(*updated);
                                if group != current_group {
                                    if !current_group.is_empty() {
                                        ui.add_space(10.0);
                                    }
                                    current_group = group;
                                    ui.horizontal(|ui| {
                                        ui.add_space(8.0);
                                        ui.add(Label::new(
                                            RichText::new(group)
                                                .size(10.5)
                                                .strong()
                                                .color(colors.muted.gamma_multiply(0.85)),
                                        ));
                                    });
                                    ui.add_space(3.0);
                                }

                                let selected = active.as_deref() == Some(id.as_str());
                                row(ui, &colors, id, title, selected, &mut action);
                                ui.add_space(1.0);
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

    let rect = panel_resp.response.rect;
    ui.painter().vline(
        rect.right() - 0.5,
        rect.y_range(),
        Stroke::new(1.0, colors.border.gamma_multiply(0.75)),
    );
}

fn row(
    ui: &mut Ui,
    colors: &ChatColors,
    id: &str,
    title: &str,
    selected: bool,
    action: &mut Option<RowAction>,
) {
    let height = 32.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::click());

    let pointer_inside = ui
        .ctx()
        .input(|i| i.pointer.hover_pos().is_some_and(|p| rect.contains(p)));
    let show_actions = pointer_inside;

    if ui.is_rect_visible(rect) {
        let radius = CornerRadius::same(7);
        let (hover_bg, selected_bg) = if ui.visuals().dark_mode {
            (
                Color32::from_rgb(0x1B, 0x1B, 0x21),
                Color32::from_rgb(0x22, 0x22, 0x2A),
            )
        } else {
            (
                Color32::from_rgb(0xEB, 0xEB, 0xEF),
                Color32::from_rgb(0xE4, 0xE4, 0xEA),
            )
        };

        if selected {
            ui.painter().rect_filled(rect, radius, selected_bg);
        } else if pointer_inside {
            ui.painter().rect_filled(rect, radius, hover_bg);
        }

        let inner = rect.shrink2(egui::vec2(10.0, 4.0));
        let mut child = ui.new_child(
            UiBuilder::new()
                .max_rect(inner)
                .layout(Layout::left_to_right(Align::Center)),
        );

        let title_color = if selected || pointer_inside {
            colors.assistant_text
        } else {
            colors.assistant_text.gamma_multiply(0.82)
        };

        let mut clicked_inline_button = false;

        let right_reserve = if show_actions { 44.0 } else { 4.0 };
        let title_width = (child.available_width() - right_reserve).max(40.0);
        child.allocate_ui_with_layout(
            egui::vec2(title_width, 22.0),
            Layout::left_to_right(Align::Center),
            |ui| {
                let mut text = RichText::new(title).size(13.0).color(title_color);
                if selected {
                    text = text.strong();
                }
                ui.add(Label::new(text).truncate().selectable(false));
            },
        );

        if show_actions {
            child.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                // Botão Excluir (× vetorial)
                let (del_rect, del_resp) =
                    ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::click());
                let del_color = if del_resp.hovered() {
                    ui.visuals().error_fg_color
                } else {
                    colors.muted
                };
                let c = del_rect.center();
                let r = 3.5;
                ui.painter().line_segment(
                    [egui::pos2(c.x - r, c.y - r), egui::pos2(c.x + r, c.y + r)],
                    Stroke::new(1.3, del_color),
                );
                ui.painter().line_segment(
                    [egui::pos2(c.x + r, c.y - r), egui::pos2(c.x - r, c.y + r)],
                    Stroke::new(1.3, del_color),
                );
                if del_resp.on_hover_text("Excluir conversa").clicked() {
                    *action = Some(RowAction::Delete(id.to_string()));
                    clicked_inline_button = true;
                }

                // Botão Renomear (lápis vetorial)
                let (ren_rect, ren_resp) =
                    ui.allocate_exact_size(egui::vec2(20.0, 20.0), Sense::click());
                let ren_color = if ren_resp.hovered() {
                    colors.assistant_text
                } else {
                    colors.muted
                };
                let rc = ren_rect.center();
                ui.painter().line_segment(
                    [
                        egui::pos2(rc.x - 3.5, rc.y + 3.5),
                        egui::pos2(rc.x + 3.5, rc.y - 3.5),
                    ],
                    Stroke::new(1.4, ren_color),
                );
                ui.painter().circle_filled(
                    egui::pos2(rc.x - 4.0, rc.y + 4.0),
                    0.9,
                    ren_color,
                );
                if ren_resp.on_hover_text("Renomear conversa").clicked() {
                    *action = Some(RowAction::Rename(id.to_string()));
                    clicked_inline_button = true;
                }
            });
        }

        if response.clicked() && !clicked_inline_button {
            *action = Some(RowAction::Select(id.to_string()));
        }
    }

    response.context_menu(|ui| {
        if ui.button("Renomear").clicked() {
            *action = Some(RowAction::Rename(id.to_string()));
            ui.close();
        }
        if ui.button("Excluir").clicked() {
            *action = Some(RowAction::Delete(id.to_string()));
            ui.close();
        }
    });
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

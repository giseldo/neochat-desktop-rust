use super::NeoChat;
use crate::storage::ThemeChoice;
use egui::{Align2, Button, Color32, ComboBox, RichText, TextEdit, Window};

pub(super) fn settings_window(app: &mut NeoChat, ctx: &egui::Context) {
    if !app.settings_open {
        return;
    }

    let mut open = true;
    let mut save = false;
    let mut test = false;
    let mut close = false;
    let pending = app.test_pending;
    let status = app.test_status.clone();
    let models = app.model_list();

    Window::new("Configurações")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_width(490.0)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.label(RichText::new("Chave da API Mistral").strong());
            ui.add(
                TextEdit::singleline(&mut app.settings_key)
                    .password(true)
                    .desired_width(f32::INFINITY)
                    .hint_text("sua chave da API"),
            );
            ui.add_space(2.0);
            ui.hyperlink_to(
                "Obter uma chave no console da Mistral",
                "https://console.mistral.ai/api-keys",
            );
            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);

            ui.label(RichText::new("Modelo").strong());
            ComboBox::from_id_salt("settings_model")
                .width(455.0)
                .selected_text(app.settings_model.clone())
                .show_ui(ui, |ui| {
                    for model in &models {
                        ui.selectable_value(&mut app.settings_model, model.clone(), model);
                    }
                });

            ui.add_space(10.0);
            ui.label(RichText::new("Tema").strong());
            ui.horizontal(|ui| {
                for choice in [
                    ThemeChoice::Sistema,
                    ThemeChoice::Claro,
                    ThemeChoice::Escuro,
                ] {
                    ui.radio_value(&mut app.settings_theme, choice, choice.label());
                }
            });

            ui.add_space(6.0);
            ui.label(
                RichText::new("A chave fica salva criptografada com o Windows (DPAPI).")
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );

            ui.add_space(14.0);
            ui.horizontal(|ui| {
                if ui
                    .add(Button::new(RichText::new("Salvar").strong()))
                    .clicked()
                {
                    save = true;
                }
                if ui.button("Cancelar").clicked() {
                    close = true;
                }
                let test_label = if pending {
                    "Testando..."
                } else {
                    "Testar conexão"
                };
                if ui.add_enabled(!pending, Button::new(test_label)).clicked() {
                    test = true;
                }
            });

            if let Some((ok, message)) = &status {
                ui.add_space(6.0);
                let color = if *ok {
                    Color32::from_rgb(0x3F, 0xB9, 0x50)
                } else {
                    ui.visuals().error_fg_color
                };
                ui.label(RichText::new(message).small().color(color));
            }

            ui.add_space(10.0);
            ui.separator();
            ui.label(
                RichText::new(format!(
                    "NeoChat {} · dados em {}",
                    env!("CARGO_PKG_VERSION"),
                    crate::storage::data_dir().display()
                ))
                .small()
                .color(ui.visuals().weak_text_color()),
            );
        });

    app.settings_open = open && !close;
    if test {
        app.start_test(ctx);
    }
    if save {
        app.apply_settings();
    }
}

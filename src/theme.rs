use crate::storage::ThemeChoice;
use egui::{
    Color32, Context, CornerRadius, FontData, FontFamily, FontId, FontInsert, FontPriority,
    InsertFontFamily, Margin, Stroke, SystemTheme, TextStyle, Theme, ThemePreference, Vec2,
    ViewportCommand,
};
use std::collections::BTreeMap;

pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x4C, 0xC2, 0xFF);
pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(0x00, 0x67, 0xC0);

pub fn preference(choice: ThemeChoice) -> ThemePreference {
    match choice {
        ThemeChoice::Sistema => ThemePreference::System,
        ThemeChoice::Claro => ThemePreference::Light,
        ThemeChoice::Escuro => ThemePreference::Dark,
    }
}

pub fn apply(ctx: &Context, choice: ThemeChoice) {
    ctx.set_theme(preference(choice));
    ctx.style_mut_of(Theme::Dark, |style| tune(style, true));
    ctx.style_mut_of(Theme::Light, |style| tune(style, false));
}

pub fn sync_title_bar(ctx: &Context, choice: ThemeChoice) {
    let theme = match choice {
        ThemeChoice::Sistema => SystemTheme::SystemDefault,
        ThemeChoice::Claro => SystemTheme::Light,
        ThemeChoice::Escuro => SystemTheme::Dark,
    };
    ctx.send_viewport_cmd(ViewportCommand::SetTheme(theme));
}

pub fn install_fonts(ctx: &Context) {
    for path in [
        "C:\\Windows\\Fonts\\SegUIVar.ttf",
        "C:\\Windows\\Fonts\\segoeui.ttf",
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            ctx.add_font(FontInsert::new(
                "Segoe UI",
                FontData::from_owned(bytes),
                vec![InsertFontFamily {
                    family: FontFamily::Proportional,
                    priority: FontPriority::Highest,
                }],
            ));
            break;
        }
    }

    if let Ok(bytes) = std::fs::read("C:\\Windows\\Fonts\\CascadiaMono.ttf") {
        ctx.add_font(FontInsert::new(
            "Cascadia Mono",
            FontData::from_owned(bytes),
            vec![InsertFontFamily {
                family: FontFamily::Monospace,
                priority: FontPriority::Highest,
            }],
        ));
    }
}

fn tune(style: &mut egui::Style, dark: bool) {
    style.visuals = if dark {
        visuals_dark()
    } else {
        visuals_light()
    };
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.window_margin = Margin::same(14);
    style.spacing.menu_margin = Margin::same(8);
    style.spacing.interact_size.y = 30.0;
    style.spacing.combo_width = 180.0;
    style.spacing.scroll.bar_width = 10.0;
    style.spacing.scroll.floating = true;
    style.animation_time = 0.12;
    style.text_styles = BTreeMap::from([
        (
            TextStyle::Heading,
            FontId::new(21.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(15.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Small,
            FontId::new(12.0, FontFamily::Proportional),
        ),
        (
            TextStyle::Monospace,
            FontId::new(13.5, FontFamily::Monospace),
        ),
    ]);
}

fn visuals_dark() -> egui::Visuals {
    let mut v = egui::Visuals::dark();
    let text = Color32::from_rgb(0xEC, 0xEC, 0xF1);
    let muted = Color32::from_rgb(0xA6, 0xA6, 0xB2);
    let border = Color32::from_rgb(0x3A, 0x3A, 0x44);

    v.panel_fill = Color32::from_rgb(0x18, 0x18, 0x1D);
    v.window_fill = Color32::from_rgb(0x23, 0x23, 0x2A);
    v.extreme_bg_color = Color32::from_rgb(0x12, 0x12, 0x16);
    v.faint_bg_color = Color32::from_rgb(0x2A, 0x2A, 0x32);
    v.code_bg_color = Color32::from_rgb(0x12, 0x12, 0x16);
    v.override_text_color = Some(text);
    v.weak_text_color = Some(muted);
    v.hyperlink_color = ACCENT_DARK;
    v.error_fg_color = Color32::from_rgb(0xFF, 0x8A, 0x8A);
    v.warn_fg_color = Color32::from_rgb(0xF7, 0xC9, 0x4C);
    v.selection.bg_fill = Color32::from_rgb(0x00, 0x5F, 0xB8);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_stroke = Stroke::new(1.0, border);

    widgets(
        &mut v.widgets,
        Color32::from_rgb(0x2C, 0x2C, 0x35),
        Color32::from_rgb(0x35, 0x35, 0x3F),
        Color32::from_rgb(0x40, 0x40, 0x4C),
        border,
        text,
    );
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(0x1E, 0x1E, 0x24);
    v.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(0x1E, 0x1E, 0x24);
    v
}

fn visuals_light() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    let text = Color32::from_rgb(0x1B, 0x1B, 0x1F);
    let muted = Color32::from_rgb(0x5F, 0x5F, 0x69);
    let border = Color32::from_rgb(0xDA, 0xDA, 0xE0);

    v.panel_fill = Color32::from_rgb(0xF3, 0xF3, 0xF6);
    v.window_fill = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    v.extreme_bg_color = Color32::from_rgb(0xEA, 0xEA, 0xEF);
    v.faint_bg_color = Color32::from_rgb(0xED, 0xED, 0xF2);
    v.code_bg_color = Color32::from_rgb(0xF4, 0xF4, 0xF8);
    v.override_text_color = Some(text);
    v.weak_text_color = Some(muted);
    v.hyperlink_color = ACCENT_LIGHT;
    v.error_fg_color = Color32::from_rgb(0xB3, 0x26, 0x1A);
    v.warn_fg_color = Color32::from_rgb(0x8A, 0x63, 0x00);
    v.selection.bg_fill = Color32::from_rgb(0x00, 0x78, 0xD4);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_stroke = Stroke::new(1.0, border);

    widgets(
        &mut v.widgets,
        Color32::from_rgb(0xFC, 0xFC, 0xFE),
        Color32::from_rgb(0xF2, 0xF2, 0xF6),
        Color32::from_rgb(0xE6, 0xE6, 0xEC),
        border,
        text,
    );
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(0xFA, 0xFA, 0xFC);
    v.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(0xFA, 0xFA, 0xFC);
    v
}

fn widgets(
    widgets: &mut egui::style::Widgets,
    fill: Color32,
    hover: Color32,
    active: Color32,
    border: Color32,
    text: Color32,
) {
    let apply = |w: &mut egui::style::WidgetVisuals, bg: Color32, stroke: Color32| {
        w.bg_fill = bg;
        w.weak_bg_fill = bg;
        w.bg_stroke = Stroke::new(1.0, stroke);
        w.fg_stroke = Stroke::new(1.0, text);
        w.corner_radius = CornerRadius::same(8);
    };

    apply(&mut widgets.inactive, fill, border);
    apply(&mut widgets.hovered, hover, border);
    apply(&mut widgets.active, active, ACCENT_LIGHT);
    apply(&mut widgets.open, hover, border);
    widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);
    widgets.noninteractive.corner_radius = CornerRadius::same(8);
}

#[derive(Debug, Clone)]
pub struct ChatColors {
    pub background: Color32,
    pub surface: Color32,
    pub border: Color32,
    pub user_bubble: Color32,
    pub user_text: Color32,
    pub assistant_bubble: Color32,
    pub assistant_text: Color32,
    pub assistant_border: Color32,
    pub muted: Color32,
    pub accent: Color32,
}

pub fn chat_colors(theme: Theme) -> ChatColors {
    match theme {
        Theme::Dark => ChatColors {
            background: Color32::from_rgb(0x14, 0x14, 0x18),
            surface: Color32::from_rgb(0x23, 0x23, 0x2A),
            border: Color32::from_rgb(0x33, 0x33, 0x3D),
            user_bubble: Color32::from_rgb(0x00, 0x5F, 0xB8),
            user_text: Color32::WHITE,
            assistant_bubble: Color32::from_rgb(0x26, 0x26, 0x2E),
            assistant_text: Color32::from_rgb(0xEC, 0xEC, 0xF1),
            assistant_border: Color32::from_rgb(0x34, 0x34, 0x3F),
            muted: Color32::from_rgb(0xA6, 0xA6, 0xB2),
            accent: ACCENT_DARK,
        },
        Theme::Light => ChatColors {
            background: Color32::from_rgb(0xFA, 0xFA, 0xFC),
            surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            border: Color32::from_rgb(0xE1, 0xE1, 0xE7),
            user_bubble: Color32::from_rgb(0x00, 0x78, 0xD4),
            user_text: Color32::WHITE,
            assistant_bubble: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            assistant_text: Color32::from_rgb(0x1B, 0x1B, 0x1F),
            assistant_border: Color32::from_rgb(0xE1, 0xE1, 0xE7),
            muted: Color32::from_rgb(0x5F, 0x5F, 0x69),
            accent: ACCENT_LIGHT,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_choice_maps_to_preference() {
        assert!(matches!(
            preference(ThemeChoice::Sistema),
            ThemePreference::System
        ));
        assert!(matches!(
            preference(ThemeChoice::Claro),
            ThemePreference::Light
        ));
        assert!(matches!(
            preference(ThemeChoice::Escuro),
            ThemePreference::Dark
        ));
    }
}

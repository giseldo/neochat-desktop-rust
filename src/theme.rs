use crate::storage::ThemeChoice;
use egui::{
    Color32, Context, CornerRadius, FontData, FontFamily, FontId, FontInsert, FontPriority,
    InsertFontFamily, Margin, Stroke, SystemTheme, TextStyle, Theme, ThemePreference, Vec2,
    ViewportCommand,
};
use std::collections::BTreeMap;

pub const ACCENT_DARK: Color32 = Color32::from_rgb(0x63, 0x84, 0xFF);
pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(0x2B, 0x4C, 0x7E);
pub const MAX_CHAT_WIDTH: f32 = 740.0;

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

    if let Ok(bytes) = std::fs::read("C:\\Windows\\Fonts\\seguisym.ttf") {
        ctx.add_font(FontInsert::new(
            "Segoe UI Symbol",
            FontData::from_owned(bytes),
            vec![InsertFontFamily {
                family: FontFamily::Proportional,
                priority: FontPriority::Lowest,
            }],
        ));
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
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.window_margin = Margin::same(18);
    style.spacing.menu_margin = Margin::same(8);
    style.spacing.interact_size.y = 30.0;
    style.spacing.combo_width = 170.0;
    style.spacing.combo_height = 420.0;
    style.spacing.scroll.bar_width = 6.0;
    style.spacing.scroll.floating = false;
    style.spacing.scroll.dormant_background_opacity = 0.15;
    style.spacing.scroll.active_background_opacity = 0.3;
    style.spacing.scroll.interact_background_opacity = 0.45;
    style.spacing.scroll.dormant_handle_opacity = 0.55;
    style.spacing.scroll.active_handle_opacity = 0.8;
    style.spacing.scroll.interact_handle_opacity = 1.0;
    style.animation_time = 0.14;
    style.text_styles = BTreeMap::from([
        (
            TextStyle::Heading,
            FontId::new(20.0, FontFamily::Proportional),
        ),
        (TextStyle::Body, FontId::new(15.0, FontFamily::Proportional)),
        (
            TextStyle::Button,
            FontId::new(13.5, FontFamily::Proportional),
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
    let text = Color32::from_rgb(0xEC, 0xEC, 0xF0);
    let muted = Color32::from_rgb(0x8E, 0x8E, 0x9E);
    let border = Color32::from_rgb(0x2A, 0x2A, 0x32);

    v.panel_fill = Color32::from_rgb(0x14, 0x14, 0x17);
    v.window_fill = Color32::from_rgb(0x1B, 0x1B, 0x20);
    v.extreme_bg_color = Color32::from_rgb(0x18, 0x18, 0x1C);
    v.faint_bg_color = Color32::from_rgb(0x20, 0x20, 0x26);
    v.code_bg_color = Color32::from_rgb(0x10, 0x10, 0x13);
    v.override_text_color = Some(text);
    v.weak_text_color = Some(muted);
    v.hyperlink_color = ACCENT_DARK;
    v.error_fg_color = Color32::from_rgb(0xF8, 0x71, 0x71);
    v.warn_fg_color = Color32::from_rgb(0xFB, 0xBF, 0x24);
    v.selection.bg_fill = Color32::from_rgb(0x2B, 0x3A, 0x67);
    v.selection.stroke = Stroke::new(1.0, Color32::WHITE);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_stroke = Stroke::new(1.0, border);

    widgets(
        &mut v.widgets,
        Color32::from_rgb(0x1E, 0x1E, 0x24),
        Color32::from_rgb(0x26, 0x26, 0x2E),
        Color32::from_rgb(0x2E, 0x2E, 0x38),
        border,
        text,
    );
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(0x18, 0x18, 0x1C);
    v.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(0x18, 0x18, 0x1C);
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;
    v
}

fn visuals_light() -> egui::Visuals {
    let mut v = egui::Visuals::light();
    let text = Color32::from_rgb(0x1A, 0x1A, 0x1E);
    let muted = Color32::from_rgb(0x78, 0x78, 0x85);
    let border = Color32::from_rgb(0xE8, 0xE8, 0xED);

    v.panel_fill = Color32::from_rgb(0xFC, 0xFC, 0xFD);
    v.window_fill = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    v.extreme_bg_color = Color32::from_rgb(0xFF, 0xFF, 0xFF);
    v.faint_bg_color = Color32::from_rgb(0xF3, 0xF3, 0xF6);
    v.code_bg_color = Color32::from_rgb(0xF4, 0xF4, 0xF7);
    v.override_text_color = Some(text);
    v.weak_text_color = Some(muted);
    v.hyperlink_color = ACCENT_LIGHT;
    v.error_fg_color = Color32::from_rgb(0xDC, 0x26, 0x26);
    v.warn_fg_color = Color32::from_rgb(0xD9, 0x77, 0x06);
    v.selection.bg_fill = Color32::from_rgb(0xD8, 0xE2, 0xF8);
    v.selection.stroke = Stroke::new(1.0, text);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_stroke = Stroke::new(1.0, border);

    widgets(
        &mut v.widgets,
        Color32::from_rgb(0xF5, 0xF5, 0xF8),
        Color32::from_rgb(0xEC, 0xEC, 0xF1),
        Color32::from_rgb(0xE3, 0xE3, 0xEA),
        border,
        text,
    );
    v.widgets.noninteractive.bg_fill = Color32::from_rgb(0xF7, 0xF7, 0xFA);
    v.widgets.noninteractive.weak_bg_fill = Color32::from_rgb(0xF7, 0xF7, 0xFA);
    v.widgets.noninteractive.bg_stroke = Stroke::NONE;
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
    let apply = |w: &mut egui::style::WidgetVisuals, bg: Color32, stroke: Stroke| {
        w.bg_fill = bg;
        w.weak_bg_fill = bg;
        w.bg_stroke = stroke;
        w.fg_stroke = Stroke::new(1.0, text);
        w.corner_radius = CornerRadius::same(8);
    };

    apply(&mut widgets.inactive, fill, Stroke::NONE);
    apply(&mut widgets.hovered, hover, Stroke::new(1.0, border));
    apply(&mut widgets.active, active, Stroke::new(1.0, border));
    apply(&mut widgets.open, hover, Stroke::new(1.0, border));
    widgets.noninteractive.corner_radius = CornerRadius::same(8);
}

#[derive(Debug, Clone)]
pub struct ChatColors {
    pub background: Color32,
    pub sidebar_bg: Color32,
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
            background: Color32::from_rgb(0x14, 0x14, 0x17),
            sidebar_bg: Color32::from_rgb(0x10, 0x10, 0x13),
            surface: Color32::from_rgb(0x1C, 0x1C, 0x21),
            border: Color32::from_rgb(0x27, 0x27, 0x2F),
            user_bubble: Color32::from_rgb(0x22, 0x22, 0x29),
            user_text: Color32::from_rgb(0xF0, 0xF0, 0xF4),
            assistant_bubble: Color32::from_rgb(0x18, 0x18, 0x1D),
            assistant_text: Color32::from_rgb(0xEC, 0xEC, 0xF0),
            assistant_border: Color32::from_rgb(0x25, 0x25, 0x2D),
            muted: Color32::from_rgb(0x88, 0x88, 0x96),
            accent: ACCENT_DARK,
        },
        Theme::Light => ChatColors {
            background: Color32::from_rgb(0xFC, 0xFC, 0xFD),
            sidebar_bg: Color32::from_rgb(0xF4, 0xF4, 0xF6),
            surface: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            border: Color32::from_rgb(0xE6, 0xE6, 0xEC),
            user_bubble: Color32::from_rgb(0xF1, 0xF1, 0xF4),
            user_text: Color32::from_rgb(0x18, 0x18, 0x1C),
            assistant_bubble: Color32::from_rgb(0xFF, 0xFF, 0xFF),
            assistant_text: Color32::from_rgb(0x1A, 0x1A, 0x1E),
            assistant_border: Color32::from_rgb(0xEA, 0xEA, 0xF0),
            muted: Color32::from_rgb(0x78, 0x78, 0x85),
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

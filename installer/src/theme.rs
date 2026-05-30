//! Itasha.Corp CRT-IC v2 brand theme (wired-noir) for the installer — the same
//! visual language as the repo banners: void phosphor screen, monitor bezel,
//! scanlines, mecha-HUD corner brackets, NERV-style wordmark, kanji watermark,
//! per-app voice colour. Influences (Lain / GitS / Akira / Eva / Gundam / JDM /
//! Gharliera / antireal) are expressed as motifs, never literal asset lifts.
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};

// --- wired-noir base palette (brand DEFAULT) ---
pub const VOID: Color32 = Color32::from_rgb(0x07, 0x0A, 0x0C); // CRT screen interior
pub const PANEL: Color32 = Color32::from_rgb(0x0E, 0x14, 0x17); // raised panel
pub const BEZEL: Color32 = Color32::from_rgb(0x1A, 0x24, 0x2B); // monitor bezel
pub const HAIRLINE: Color32 = Color32::from_rgb(0x2A, 0x3A, 0x42);
pub const STRIP: Color32 = Color32::from_rgb(0x3A, 0x4A, 0x52);
pub const TEXT: Color32 = Color32::from_rgb(0xC8, 0xD6, 0xDC); // off-white phosphor
pub const MUTED: Color32 = Color32::from_rgb(0x5A, 0x6B, 0x73);
pub const DIM: Color32 = Color32::from_rgb(0x4F, 0x5E, 0x66);
pub const GREEN: Color32 = Color32::from_rgb(0x6F, 0xB8, 0x9A); // muted OK
pub const AMBER: Color32 = Color32::from_rgb(0xF2, 0xB3, 0x3D); // warning accent (sparing)
pub const RED: Color32 = Color32::from_rgb(0xFF, 0x3B, 0x30); // alarms ONLY

/// Parse a `#RRGGBB` brand hex into a Color32 (falls back to C0PL4ND violet).
pub fn hex(s: &str) -> Color32 {
    let s = s.trim().trim_start_matches('#');
    let p = |i: usize, d: u8| {
        s.get(i..i + 2)
            .and_then(|h| u8::from_str_radix(h, 16).ok())
            .unwrap_or(d)
    };
    Color32::from_rgb(p(0, 0xB4), p(2, 0x8C), p(4, 0xE8))
}

/// The per-app voice colour (banner accent).
pub fn voice() -> Color32 {
    hex(crate::config::VOICE_HEX)
}

/// A dimmed mix of a colour toward the void (for subtle fills/glows).
pub fn dimmed(c: Color32, t: f32) -> Color32 {
    let m = |a: u8, b: u8| (a as f32 * (1.0 - t) + b as f32 * t) as u8;
    Color32::from_rgb(m(c.r(), VOID.r()), m(c.g(), VOID.g()), m(c.b(), VOID.b()))
}

/// Install global egui style: dark, monospace, brand colours.
pub fn apply(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let v = voice();
    let vis = &mut style.visuals;
    vis.dark_mode = true;
    vis.override_text_color = Some(TEXT);
    vis.panel_fill = VOID;
    vis.window_fill = VOID;
    vis.extreme_bg_color = Color32::from_rgb(0x05, 0x07, 0x09);
    vis.faint_bg_color = PANEL;
    vis.widgets.noninteractive.bg_fill = PANEL;
    vis.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    vis.widgets.inactive.bg_fill = PANEL;
    vis.widgets.inactive.weak_bg_fill = PANEL;
    vis.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    vis.widgets.inactive.bg_stroke = Stroke::new(1.0, HAIRLINE);
    vis.widgets.hovered.bg_fill = dimmed(v, 0.78);
    vis.widgets.hovered.weak_bg_fill = dimmed(v, 0.82);
    vis.widgets.hovered.bg_stroke = Stroke::new(1.2, v);
    vis.widgets.hovered.fg_stroke = Stroke::new(1.2, TEXT);
    vis.widgets.active.bg_fill = dimmed(v, 0.6);
    vis.widgets.active.weak_bg_fill = dimmed(v, 0.66);
    vis.widgets.active.bg_stroke = Stroke::new(1.4, v);
    vis.widgets.active.fg_stroke = Stroke::new(1.4, TEXT);
    vis.selection.bg_fill = dimmed(v, 0.7);
    vis.selection.stroke = Stroke::new(1.0, v);
    vis.hyperlink_color = v;
    vis.widgets.noninteractive.rounding = egui::Rounding::same(3.0);
    vis.widgets.inactive.rounding = egui::Rounding::same(3.0);
    vis.widgets.hovered.rounding = egui::Rounding::same(3.0);
    vis.widgets.active.rounding = egui::Rounding::same(3.0);

    // monospace everywhere (terminal feel); the wordmark uses wide tracking.
    use egui::{FontFamily, TextStyle};
    style.text_styles = [
        (TextStyle::Heading, FontId::new(26.0, FontFamily::Monospace)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Monospace)),
        (TextStyle::Monospace, FontId::new(13.5, FontFamily::Monospace)),
        (TextStyle::Button, FontId::new(14.0, FontFamily::Monospace)),
        (TextStyle::Small, FontId::new(11.0, FontFamily::Monospace)),
    ]
    .into();
    style.spacing.button_padding = Vec2::new(14.0, 8.0);
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    ctx.set_style(style);
}

/// Paint the CRT chrome over a rect: bezel frame, scanlines, faint vignette,
/// the four mecha-HUD corner brackets, a targeting reticle, the kanji
/// watermark, and the bezel status strip. Drawn UNDER the wizard content.
pub fn paint_chrome(p: &egui::Painter, rect: Rect, t_seconds: f64) {
    let v = voice();
    // screen fill
    p.rect_filled(rect, 8.0, VOID);

    // scanlines (very faint)
    let mut y = rect.top();
    while y < rect.bottom() {
        p.hline(
            rect.x_range(),
            y,
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 0, 0, 38)),
        );
        y += 3.0;
    }

    // faint vignette via darkened edge bands
    let edge = Color32::from_rgba_unmultiplied(0, 0, 0, 60);
    let b = 26.0;
    p.rect_filled(
        Rect::from_min_max(rect.left_top(), Pos2::new(rect.right(), rect.top() + b)),
        0.0,
        edge,
    );
    p.rect_filled(
        Rect::from_min_max(Pos2::new(rect.left(), rect.bottom() - b), rect.right_bottom()),
        0.0,
        edge,
    );

    // kanji watermark, top-right (faint)
    p.text(
        Pos2::new(rect.right() - 30.0, rect.top() + 34.0),
        egui::Align2::RIGHT_CENTER,
        crate::config::KANJI,
        FontId::proportional(56.0),
        Color32::from_rgba_unmultiplied(v.r(), v.g(), v.b(), 26),
    );

    // four mecha-HUD corner brackets
    let bracket = |c: Pos2, dx: f32, dy: f32| {
        let s = Stroke::new(1.4, dimmed(v, 0.25));
        let len = 22.0;
        p.line_segment([c, Pos2::new(c.x + dx * len, c.y)], s);
        p.line_segment([c, Pos2::new(c.x, c.y + dy * len)], s);
    };
    let m = 14.0;
    bracket(Pos2::new(rect.left() + m, rect.top() + m), 1.0, 1.0);
    bracket(Pos2::new(rect.right() - m, rect.top() + m), -1.0, 1.0);
    bracket(Pos2::new(rect.left() + m, rect.bottom() - m), 1.0, -1.0);
    bracket(Pos2::new(rect.right() - m, rect.bottom() - m), -1.0, -1.0);

    // targeting reticle (GitS/mecha HUD touch), small, top-left of content
    let ret = Pos2::new(rect.left() + 40.0, rect.bottom() - 40.0);
    let pulse = 0.5 + 0.5 * ((t_seconds * 1.6).sin() as f32);
    p.circle_stroke(ret, 6.0, Stroke::new(1.0, dimmed(v, 0.2)));
    p.circle_filled(ret, 1.6, Color32::from_rgba_unmultiplied(v.r(), v.g(), v.b(), (120.0 + 110.0 * pulse) as u8));

    // bezel border + status strip
    p.rect_stroke(rect, 8.0, Stroke::new(1.5, BEZEL));
    p.rect_stroke(rect.shrink(3.0), 6.0, Stroke::new(1.0, HAIRLINE));
}

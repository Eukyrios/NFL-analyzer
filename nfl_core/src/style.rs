//! Design tokens as plain RGB(A) tuples. The app converts them to egui colors.
//! Field, player, result and role values match `timeline/viewer.html`.

pub type Rgb = (u8, u8, u8);
pub type Rgba = (u8, u8, u8, u8);

/// Parses `#RRGGBB` (used by tests and the team table checks).
pub fn hex(s: &str) -> Option<Rgb> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(s, 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

// ---- App chrome ----
pub const BG: Rgb = (0x0B, 0x15, 0x26);
pub const SURFACE: Rgb = (0x13, 0x21, 0x3A);
pub const SURFACE_RAISED: Rgb = (0x1B, 0x2C, 0x4B);
pub const BORDER: Rgb = (0x2A, 0x3B, 0x5C);
pub const TEXT: Rgb = (0xFF, 0xFF, 0xFF);
pub const TEXT_MUTED: Rgb = (0x9A, 0xA8, 0xBD);
pub const NAVY: Rgb = (0x01, 0x33, 0x69);
pub const RED: Rgb = (0xD5, 0x0A, 0x0A);
/// Neutral bars (block type, dropback type).
pub const BAR_NEUTRAL: Rgb = (0x3C, 0x6F, 0xB4);

// ---- Field ----
pub const FIELD_OUTSIDE: Rgb = (0x1D, 0x5A, 0x28);
pub const GRASS_A: Rgb = (0x2B, 0x7A, 0x37);
pub const GRASS_B: Rgb = (0x2F, 0x84, 0x3C);
pub const LINE: Rgba = (255, 255, 255, 204); // 80 %
pub const YARD_NUMBER: Rgba = (255, 255, 255, 191); // 75 %
pub const LOS: Rgb = (0x3B, 0x82, 0xF6);
pub const FIRST_DOWN: Rgb = (0xFA, 0xCC, 0x15);
pub const ENDZONE_BADGE: Rgba = (255, 255, 255, 235); // 92 %
pub const ARROW: Rgba = (255, 255, 255, 230); // 90 %
pub const BALL: Rgb = (0x8B, 0x4A, 0x1C);
pub const BALL_TRAIL: Rgb = (0xF5, 0xDE, 0xB3);
pub const DARK_TEXT: Rgb = (0x11, 0x11, 0x11);
pub const LIGHT_STROKE: Rgb = (0x33, 0x33, 0x33);
pub const ROLE_STROKE: Rgb = (0x0D, 0x11, 0x17);
pub const UNKNOWN_TEAM: Rgb = (0x6E, 0x76, 0x81);
pub const UNKNOWN_ROLE: Rgb = (0x9C, 0xA3, 0xAF);

// ---- Meaning colors ----
pub const RESULT_C: Rgb = (0x3F, 0xB9, 0x50);
pub const RESULT_I: Rgb = (0x8B, 0x94, 0x9E);
pub const RESULT_S: Rgb = (0xF8, 0x51, 0x49);
pub const RESULT_IN: Rgb = (0xF0, 0x88, 0x3E);
pub const RESULT_R: Rgb = (0x58, 0xA6, 0xFF);

pub const ROLE_PASS: Rgb = (0xFA, 0xCC, 0x15);
pub const ROLE_BLOCK: Rgb = (0x60, 0xA5, 0xFA);
pub const ROLE_ROUTE: Rgb = (0x2D, 0xD4, 0xBF);
pub const ROLE_RUSH: Rgb = (0xF8, 0x71, 0x71);
pub const ROLE_COVERAGE: Rgb = (0xC0, 0x84, 0xFC);

pub fn result_color(code: &str) -> Rgb {
    match code {
        "C" => RESULT_C,
        "I" => RESULT_I,
        "S" => RESULT_S,
        "IN" => RESULT_IN,
        "R" => RESULT_R,
        _ => TEXT_MUTED,
    }
}

pub fn role_color(pff_role: &str) -> Rgb {
    match pff_role {
        "Pass" => ROLE_PASS,
        "Pass Block" => ROLE_BLOCK,
        "Pass Route" => ROLE_ROUTE,
        "Pass Rush" => ROLE_RUSH,
        "Coverage" => ROLE_COVERAGE,
        _ => UNKNOWN_ROLE,
    }
}

// ---- Type, spacing, controls ----
pub struct Sizes {
    pub heading: f32,
    pub body: f32,
    pub small: f32,
    pub touch_target: f32,
}
pub const DESKTOP: Sizes = Sizes { heading: 20.0, body: 14.0, small: 12.0, touch_target: 28.0 };
pub const PHONE: Sizes = Sizes { heading: 22.0, body: 16.0, small: 13.0, touch_target: 44.0 };
pub const SPACE: [f32; 5] = [4.0, 8.0, 12.0, 16.0, 24.0];
pub const RADIUS_PANEL: f32 = 8.0;
pub const RADIUS_BUTTON: f32 = 6.0;

// ---- Glyphs (egui's built-in emoji/icon font) ----
pub const ICON_PLAY: &str = "▶";
pub const ICON_PAUSE: &str = "⏸";
pub const ICON_PREV: &str = "⏮";
pub const ICON_NEXT: &str = "⏭";
pub const ICON_BACK: &str = "⬅";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_hex() {
        assert_eq!(hex("#013369"), Some(NAVY));
        assert_eq!(hex("013369"), None);
    }
}

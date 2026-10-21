//! Team logos from `teams.logo_png`, uploaded as textures once at startup.
//! Missing logos fall back to a team-colored circle with the abbreviation.

use std::collections::HashMap;

use egui::{Align2, Color32, ColorImage, FontId, Painter, Rect, Response, Sense, TextureHandle, TextureOptions, Ui};
use nfl_core::dal::Db;

use crate::theme;

#[derive(Default)]
pub struct Logos {
    tex: HashMap<String, TextureHandle>,
}

impl Logos {
    pub fn load(ctx: &egui::Context, db: Option<&Db>) -> Self {
        let mut tex = HashMap::new();
        let Some(teams) = db.and_then(|db| db.teams().ok()) else { return Logos { tex } };
        for t in teams {
            let Some(png) = t.logo_png else { continue };
            match image::load_from_memory(&png) {
                Ok(img) => {
                    let rgba = img.to_rgba8();
                    let size = [rgba.width() as usize, rgba.height() as usize];
                    let ci = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
                    tex.insert(t.abbr.clone(), ctx.load_texture(format!("logo-{}", t.abbr), ci, TextureOptions::LINEAR));
                }
                Err(e) => eprintln!("warning: logo {} unreadable: {e}", t.abbr),
            }
        }
        Logos { tex }
    }

    pub fn has(&self, abbr: &str) -> bool {
        self.tex.contains_key(abbr)
    }

    /// Draws the logo fitted into `rect` (square), or the color-circle fallback.
    pub fn paint(&self, p: &Painter, rect: Rect, abbr: &str, opacity: f32) {
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        if let Some(t) = self.tex.get(abbr) {
            p.image(t.id(), rect, uv, Color32::WHITE.gamma_multiply(opacity));
        } else {
            let col = nfl_core::teams::primary(abbr);
            let r = rect.width().min(rect.height()) / 2.0;
            p.circle_filled(rect.center(), r, theme::c(col).gamma_multiply(opacity));
            p.text(rect.center(), Align2::CENTER_CENTER, abbr, FontId::proportional(r * 0.75), theme::on(col).gamma_multiply(opacity));
        }
    }

    pub fn widget(&self, ui: &mut Ui, abbr: &str, size: f32) -> Response {
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(size, size), Sense::hover());
        self.paint(ui.painter(), rect, abbr, 1.0);
        resp
    }
}

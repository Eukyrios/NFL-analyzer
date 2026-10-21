//! Native NFL dropback play viewer: egui/eframe UI over `nfl_core`.

pub mod logos;
pub mod platform;
pub mod state;
pub mod theme;
mod ui;

use egui::{Color32, RichText};
use nfl_core::style as s;
use state::{AppState, Screen};

pub struct App {
    pub state: AppState,
    logos: logos::Logos,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, db: Result<nfl_core::dal::Db, String>) -> Self {
        theme::apply(&cc.egui_ctx);
        let state = AppState::new(db);
        let logos = logos::Logos::load(&cc.egui_ctx, state.db.as_ref());
        App { state, logos }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let back = ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.pointer.button_pressed(egui::PointerButton::Extra1));
        if back {
            self.state.back();
        }
        let dt = ctx.input(|i| i.stable_dt).min(0.1);
        self.state.tick(dt);

        top_bar(ctx, &mut self.state);
        if let Some(msg) = self.state.fatal.clone() {
            egui::CentralPanel::default().show(ctx, |ui| fatal(ui, &msg));
            return;
        }
        match self.state.screen().clone() {
            Screen::Filters => ui::filters::show(ctx, &mut self.state, &self.logos),
            Screen::Map => ui::team::map(ctx, &mut self.state, &self.logos),
            Screen::Team { .. } => ui::team::team(ctx, &mut self.state, &self.logos),
            Screen::Viewer { .. } => ui::viewer::show(ctx, &mut self.state, &self.logos),
            Screen::Player { .. } => ui::player::show(ctx, &mut self.state, &self.logos),
        }
        if self.state.is_playing() {
            ctx.request_repaint();
        }
    }
}

fn top_bar(ctx: &egui::Context, st: &mut AppState) {
    // Android status bar / iOS notch: fixed reserve, like egui's hello_android.
    let top = if theme::IS_PHONE { 32 } else { 0 };
    egui::TopBottomPanel::top("top")
        .frame(egui::Frame::new().fill(theme::c(s::NAVY)).inner_margin(egui::Margin { left: 12, right: 12, top: 6 + top, bottom: 6 }))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if st.can_go_back() && ui.add(egui::Button::new(RichText::new(format!("{} Back", s::ICON_BACK)).color(Color32::WHITE)).fill(theme::alpha(s::BG, 0.4))).clicked() {
                    st.back();
                }
                ui.label(RichText::new("NFL Play Viewer").font(theme::heading()).color(Color32::WHITE).strong());
                ui.add_space(16.0);
                let root = st.screen().clone();
                for (label, scr) in [("Plays", Screen::Filters), ("Teams", Screen::Map)] {
                    let sel = std::mem::discriminant(&root) == std::mem::discriminant(&scr) && !st.can_go_back();
                    let b = egui::Button::new(RichText::new(label).color(Color32::WHITE))
                        .fill(if sel { theme::c(s::RED) } else { theme::alpha(s::BG, 0.35) });
                    if ui.add(b).clicked() {
                        st.set_root(scr);
                    }
                }
                if let Some(e) = st.error.clone() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button("✖").clicked() {
                            st.error = None;
                        }
                        ui.label(RichText::new(e).color(Color32::WHITE).small());
                    });
                }
            });
        });
}

fn fatal(ui: &mut egui::Ui, msg: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(80.0);
        ui.label(RichText::new("Can't open the play database").font(theme::heading()).strong());
        ui.add_space(8.0);
        ui.label(RichText::new(msg).color(theme::muted()));
        ui.add_space(8.0);
        ui.label(RichText::new("Build it with:  cargo run -p etl --release").font(theme::mono()));
    });
}

/// Desktop entry point.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn run_desktop() -> eframe::Result<()> {
    let db = platform::open_desktop();
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1400.0, 860.0]).with_min_inner_size([900.0, 560.0]).with_title("NFL Play Viewer"),
        ..Default::default()
    };
    eframe::run_native("NFL Play Viewer", opts, Box::new(move |cc| Ok(Box::new(App::new(cc, db)))))
}

#[cfg(target_os = "android")]
#[no_mangle]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info));
    let db = platform::open_android(&app);
    let opts = eframe::NativeOptions { android_app: Some(app), ..Default::default() };
    if let Err(e) = eframe::run_native("NFL Play Viewer", opts, Box::new(move |cc| Ok(Box::new(App::new(cc, db))))) {
        log::error!("eframe: {e}");
    }
}

/// iOS entry point, called from the Xcode project's `main.m` (see `ios/`).
#[cfg(target_os = "ios")]
#[no_mangle]
pub extern "C" fn nfl_app_main() {
    let db = platform::open_ios();
    let opts = eframe::NativeOptions::default();
    let _ = eframe::run_native("NFL Play Viewer", opts, Box::new(move |cc| Ok(Box::new(App::new(cc, db)))));
}

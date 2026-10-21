#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn main() -> eframe::Result<()> {
    app::run_desktop()
}

#[cfg(any(target_os = "android", target_os = "ios"))]
fn main() {}

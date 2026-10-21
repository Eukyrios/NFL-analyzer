//! Where the read-only `nfl.sqlite` lives on each platform.

use nfl_core::dal::Db;

/// Desktop: `--db PATH`, `NFL_DB`, then `data/nfl.sqlite` (cwd or repo).
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn open_desktop() -> Result<Db, String> {
    use std::path::PathBuf;
    let mut args = std::env::args().skip(1);
    let mut cli = None;
    while let Some(a) = args.next() {
        if a == "--db" {
            cli = args.next().map(PathBuf::from);
        } else if !a.starts_with('-') {
            cli = Some(PathBuf::from(a));
        }
    }
    let candidates: Vec<PathBuf> = cli
        .into_iter()
        .chain(std::env::var_os("NFL_DB").map(PathBuf::from))
        .chain([PathBuf::from("data/nfl.sqlite"), PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../data/nfl.sqlite")])
        .collect();
    let path = candidates.iter().find(|p| p.exists()).unwrap_or(&candidates[0]);
    Db::open_ro(path).map_err(|e| e.to_string())
}

/// Android: copy the APK asset to internal storage on first launch (or when
/// the schema version changed), then open it read-only.
#[cfg(target_os = "android")]
pub fn open_android(app: &winit::platform::android::activity::AndroidApp) -> Result<Db, String> {
    use std::ffi::CString;
    use std::io::{Read, Write};
    let dir = app.internal_data_path().ok_or("no internal data path")?;
    let path = dir.join("nfl.sqlite");
    let current = Db::open_ro(&path).is_ok();
    if !current {
        let name = CString::new("nfl.sqlite").map_err(|e| e.to_string())?;
        let mut asset = app.asset_manager().open(&name).ok_or("nfl.sqlite missing from APK assets")?;
        let tmp = dir.join("nfl.sqlite.copying");
        let mut out = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let n = asset.read(&mut buf).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
        drop(out);
        std::fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    }
    Db::open_ro(&path).map_err(|e| e.to_string())
}

/// iOS: the DB is a bundle resource next to the executable; open immutable.
#[cfg(target_os = "ios")]
pub fn open_ios() -> Result<Db, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let path = exe.parent().ok_or("no bundle dir")?.join("nfl.sqlite");
    Db::open_immutable(&path).map_err(|e| e.to_string())
}

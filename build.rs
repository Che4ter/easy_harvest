use image::imageops::FilterType;

fn main() {
    let out = std::env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed=assets/logo.png");

    let logo = image::open("assets/logo.png").expect("assets/logo.png missing");

    // ── Tray icons: RGBA8 (tray-icon expects raw RGBA bytes) ──────────────────
    for sz in [16u32, 22, 32] {
        let rgba = logo
            .resize_exact(sz, sz, FilterType::Lanczos3)
            .to_rgba8();
        std::fs::write(format!("{out}/tray_{sz}.rgba8"), rgba.into_raw()).unwrap();
    }

    // ── Window icon: RGBA8 at 64 × 64 ────────────────────────────────────────
    let rgba64 = logo
        .resize_exact(64, 64, FilterType::Lanczos3)
        .to_rgba8();
    std::fs::write(format!("{out}/window_64.rgba8"), rgba64.into_raw()).unwrap();
}

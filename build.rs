use image::imageops::FilterType;

fn main() {
    let out = std::env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed=assets/logo.png");

    let logo = image::open("assets/logo.png").expect("assets/logo.png missing");

    // ── Tray icons: ARGB32 (SNI spec, network/big-endian byte order) ──────────
    for sz in [16u32, 22, 32] {
        let rgba = logo
            .resize_exact(sz, sz, FilterType::Lanczos3)
            .to_rgba8();
        let argb: Vec<u8> = rgba
            .pixels()
            .flat_map(|px| [px[3], px[0], px[1], px[2]])
            .collect();
        std::fs::write(format!("{out}/tray_{sz}.argb32"), argb).unwrap();
    }

    // ── Window icon: RGBA8 at 64 × 64 ────────────────────────────────────────
    let rgba64 = logo
        .resize_exact(64, 64, FilterType::Lanczos3)
        .to_rgba8();
    std::fs::write(format!("{out}/window_64.rgba8"), rgba64.into_raw()).unwrap();
}

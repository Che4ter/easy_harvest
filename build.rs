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

    // ── Windows executable icon (.ico) ───────────────────────────────────────
    // Generate a multi-resolution .ico from the logo, then embed it as a
    // Windows resource so Explorer, taskbar, etc. show the app icon.
    let ico_path = format!("{out}/app_icon.ico");
    {
        use std::io::BufWriter;
        let file = std::fs::File::create(&ico_path).expect("create ico file");
        let mut writer = BufWriter::new(file);
        let encoder = image::codecs::ico::IcoEncoder::new(&mut writer);
        // Include 16, 32, 48, and 256 px variants for best display at all sizes.
        let sizes = [16u32, 32, 48, 256];
        let images: Vec<image::codecs::ico::IcoFrame<'_>> = sizes
            .iter()
            .map(|&sz| {
                let rgba = logo
                    .resize_exact(sz, sz, FilterType::Lanczos3)
                    .to_rgba8();
                image::codecs::ico::IcoFrame::as_png(
                    rgba.as_raw(),
                    sz,
                    sz,
                    image::ColorType::Rgba8.into(),
                )
                .expect("encode ico frame")
            })
            .collect();
        encoder.encode_images(&images).expect("write ico");
    }

    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon(&ico_path);
        res.compile().expect("winresource compile");
    }
}

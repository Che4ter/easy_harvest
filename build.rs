use image::imageops::FilterType;

fn main() {
    let out = std::env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed=assets/logo.png");

    let logo = image::open("assets/logo.png").expect("assets/logo.png missing");

    // ── Tray icon: RGBA8 at 32 × 32 (the only size tray.rs uses) ─────────────
    let tray_rgba = logo.resize_exact(32, 32, FilterType::Lanczos3).to_rgba8();
    std::fs::write(format!("{out}/tray_32.rgba8"), tray_rgba.into_raw()).unwrap();

    // ── Window icon: RGBA8 at 64 × 64 ────────────────────────────────────────
    let rgba64 = logo
        .resize_exact(64, 64, FilterType::Lanczos3)
        .to_rgba8();
    std::fs::write(format!("{out}/window_64.rgba8"), rgba64.into_raw()).unwrap();

    // ── App icon PNG at 128 × 128 (for Linux .desktop / freedesktop install) ─
    let png128 = logo.resize_exact(128, 128, FilterType::Lanczos3);
    png128.save(format!("{out}/easy_harvest_128.png")).unwrap();

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

    // ── macOS bundle icons ────────────────────────────────────────────────────
    // cargo-bundle requires PNG files at standard macOS icon sizes (16, 32, 64,
    // 128, 256, 512, 1024).  Write them to target/icons/ (already gitignored)
    // so Cargo.toml can reference them without committing binary assets.
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let icons_dir = std::path::Path::new(&manifest_dir).join("target").join("icons");
    std::fs::create_dir_all(&icons_dir).expect("create target/icons");
    for &sz in &[16u32, 32, 128, 256, 512, 1024] {
        let resized = logo.resize_exact(sz, sz, FilterType::Lanczos3);
        resized
            .save(icons_dir.join(format!("icon_{sz}.png")))
            .unwrap_or_else(|e| panic!("save icon_{sz}.png: {e}"));
    }

    // Embed the .ico as a Windows PE resource when targeting Windows,
    // regardless of the host OS (supports cross-compilation).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "1.0.0".into());
        // Windows version strings require four components (major.minor.patch.build).
        let version_quad = format!("{version}.0");
        let mut res = winresource::WindowsResource::new();
        res.set_icon(&ico_path);
        res.set("FileDescription", "Easy Harvest — Harvest time tracking desktop app");
        res.set("ProductName", "Easy Harvest");
        res.set("LegalCopyright", "https://github.com/Che4ter/easy_harvest");
        res.set("OriginalFilename", "easy_harvest.exe");
        res.set("FileVersion", &version_quad);
        res.set("ProductVersion", &version_quad);
        res.compile().expect("winresource compile");
    }
}

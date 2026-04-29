fn main() {
    println!("cargo:rerun-if-changed=assets/app_icon.ico");

    // Embed the .ico as a Windows PE resource so Explorer, taskbar, etc.
    // show the correct app icon.  All other icon assets (tray, window, macOS
    // bundle, Linux hicolor) are committed to assets/ and included directly
    // via include_bytes! in the source, so no image processing is needed here.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "1.0.0".into());
        let version_quad = format!("{version}.0");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/app_icon.ico");
        res.set("FileDescription", "Easy Harvest — Harvest time tracking desktop app");
        res.set("ProductName", "Easy Harvest");
        res.set("LegalCopyright", "https://github.com/Che4ter/easy_harvest");
        res.set("OriginalFilename", "easy_harvest.exe");
        res.set("FileVersion", &version_quad);
        res.set("ProductVersion", &version_quad);
        res.compile().expect("winresource compile");
    }
}

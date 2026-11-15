fn main() {
    println!("cargo:rerun-if-changed=hearth.manifest");
    // The exe's own icon (taskbar, Explorer) is written here: a new icon set
    // must run this again, or the old one stays baked in.
    println!("cargo:rerun-if-changed=icons");
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("hearth.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("tauri build");
}

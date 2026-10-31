fn main() {
    println!("cargo:rerun-if-changed=hearth.manifest");
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("hearth.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("tauri build");
}

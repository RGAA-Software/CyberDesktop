fn main() {
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon = manifest_dir
        .join("../app-assets/assets/app/logo/ic_cyber_monitor.ico");
    let rc = manifest_dir.join("app-icon.rc");
    println!("cargo:rerun-if-changed={}", rc.display());
    println!("cargo:rerun-if-changed={}", icon.display());
    // GPUI loads the Windows icon with MAKEINTRESOURCE(1); the ID must be numeric 1.
    let _ = embed_resource::compile("app-icon.rc", embed_resource::NONE);
}

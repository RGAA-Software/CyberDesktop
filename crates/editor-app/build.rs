fn main() {
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon = manifest_dir.join("../app-assets/assets/app/logo/ic_cyber_editor.ico");
    let rc = manifest_dir.join("app-icon.rc");
    println!("cargo:rerun-if-changed={}", rc.display());
    println!("cargo:rerun-if-changed={}", icon.display());
    // GPUI loads the Windows icon with MAKEINTRESOURCE(1); the ID must be numeric 1.
    let _ = embed_resource::compile("app-icon.rc", embed_resource::NONE);
    copy_bundled_7zip();
}

/// Copies official 64-bit `7z.dll` next to the built binary for in-process extraction.
fn copy_bundled_7zip() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let tools = std::path::Path::new(&manifest_dir)
        .join("..")
        .join("..")
        .join("tools")
        .join("7zr");
    let dest_dir = std::path::Path::new(&manifest_dir)
        .join("..")
        .join("..")
        .join("target")
        .join(&profile);

    let src = tools.join("7z.dll");
    if !src.is_file() {
        println!("cargo:warning=tools/7zr/7z.dll missing; archive extract falls back to slow Rust decoders");
        println!("cargo:warning=download 7-Zip 26.01 x64 from https://www.7-zip.org/ and copy 7z.dll to tools/7zr/");
        return;
    }
    println!("cargo:rerun-if-changed={}", src.display());
    if let Some(parent) = dest_dir.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::create_dir_all(&dest_dir);
    let dest = dest_dir.join("7z.dll");
    if let Err(error) = std::fs::copy(&src, &dest) {
        println!("cargo:warning=failed to copy 7z.dll: {error}");
    }
}

fn main() {
    copy_bundled_mpv();
}

fn copy_bundled_mpv() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let root = std::path::Path::new(&manifest_dir).join("..").join("..");
    let src = root.join("third_party").join("mpv-dev").join("libmpv-2.dll");
    let dest_dir = root.join("target").join(&profile);

    if !src.is_file() {
        println!("cargo:warning=third_party/mpv-dev/libmpv-2.dll missing; mpv demo will not run");
        return;
    }

    println!("cargo:rerun-if-changed={}", src.display());
    let _ = std::fs::create_dir_all(&dest_dir);
    let dest = dest_dir.join("libmpv-2.dll");
    if let Err(error) = std::fs::copy(&src, &dest) {
        println!("cargo:warning=failed to copy libmpv-2.dll: {error}");
    }
}


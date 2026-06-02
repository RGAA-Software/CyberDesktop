fn main() {
    #[cfg(windows)]
    {
        cc::Build::new()
            .cpp(true)
            .file("cpp/sevenzip_extract.cpp")
            .define("UNICODE", None)
            .define("_UNICODE", None)
            .flag_if_supported("/EHsc")
            .flag_if_supported("/W3")
            .compile("sevenzip_extract");
        println!("cargo:rustc-link-lib=ole32");
        println!("cargo:rustc-link-lib=shlwapi");
    }
}

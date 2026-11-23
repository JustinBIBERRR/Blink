fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/fog.m")
            .flag("-fobjc-arc")
            .compile("blink_fog");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rerun-if-changed=src/fog.m");
    }
    tauri_build::build()
}

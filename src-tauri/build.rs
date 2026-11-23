fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new().file("src/fog.m").file("src/system_events.m")
            .flag("-fobjc-arc").compile("blink_native");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rerun-if-changed=src/fog.m");
        println!("cargo:rerun-if-changed=src/system_events.m");
    }
    tauri_build::build()
}

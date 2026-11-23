fn main() {
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("src/system_events.m")
            .flag("-fobjc-arc")
            .compile("blink_system_events");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rerun-if-changed=src/system_events.m");
    }
    tauri_build::build()
}

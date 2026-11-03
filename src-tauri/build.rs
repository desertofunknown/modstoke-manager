fn main() {
    // Imported system DLLs such as version.dll and dwmapi.dll resolve from System32 only, so a copy
    // planted next to the portable exe in Downloads is never loaded.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
    }
    tauri_build::build()
}

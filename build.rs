fn main() {
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    // Keep the build timestamp current for application inputs. Cargo's default
    // package-wide scan also reruns this for docs and external browser scripts,
    // rewriting built.rs and needlessly relinking every native test binary.
    for path in [
        "build.rs",
        "Cargo.toml",
        "Cargo.lock",
        "src",
        "web/app.html",
        "web/login.html",
        "web/dist",
        "emulator-web/dist",
        "examples/emulator",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    built::write_built_file().expect("Failed to write build file");
}

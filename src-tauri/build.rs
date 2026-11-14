fn main() {
    // The PoB service's address and key are read at build time (builds::SERVICE_URL and
    // builds::SERVICE_KEY).
    println!("cargo:rerun-if-env-changed=POB_SERVICE_URL");
    println!("cargo:rerun-if-env-changed=POB_SERVICE_KEY");
    tauri_build::build()
}

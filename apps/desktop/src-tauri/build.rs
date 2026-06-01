fn main() {
    println!("cargo:rerun-if-changed=icons/icon.png");
    tauri_build::build()
}

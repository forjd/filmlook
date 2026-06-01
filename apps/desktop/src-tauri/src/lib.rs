mod commands;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_recipes,
            commands::render_preview,
            commands::export_image,
        ])
        .run(tauri::generate_context!())
        .expect("error while running filmlook desktop");
}

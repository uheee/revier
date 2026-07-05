#[tauri::command]
fn health_check() -> &'static str {
    "ok"
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![health_check])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}

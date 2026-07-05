mod commands;
mod error;
mod services;
mod state;

use services::projects::ProjectService;
use services::review::ReviewService;
use state::AppState;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let project_service = ProjectService::new(data_dir.join("projects.json"));
            let review_service = ReviewService::default();
            app.manage(AppState::new(project_service, review_service));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::projects::projects_list,
            commands::review::review_get_task,
            commands::review::review_start_analysis,
            commands::review::review_list_changed_files
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}

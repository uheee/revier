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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let project_service = ProjectService::new(data_dir.join("projects.json"));
            let review_service = ReviewService::default();
            app.manage(AppState::new(project_service, review_service));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::projects::projects_list,
            commands::projects::projects_add,
            commands::projects::projects_update,
            commands::projects::projects_remove,
            commands::projects::projects_validate_repository,
            commands::projects::projects_list_branches,
            commands::projects::projects_select_directory,
            commands::review::review_get_task,
            commands::review::review_start_analysis,
            commands::review::review_list_changed_files,
            commands::review::review_cancel_analysis,
            commands::review::review_list_authors,
            commands::review::review_get_file_overlay,
            commands::review::review_get_commit_overlay
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}

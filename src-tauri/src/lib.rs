mod commands;
mod error;
mod services;
mod state;

use services::editor_settings::EditorSettingsService;
use services::logging::LoggingService;
use services::projects::ProjectService;
use services::review::ReviewService;
use state::AppState;
use tauri::Manager;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            let logging = LoggingService::load(config_dir.join("logging.toml"));
            if let Some(warning) = logging.warning() {
                eprintln!("{warning}");
            }
            #[cfg(not(test))]
            match logging.build_plugin() {
                Ok(Some(plugin)) => {
                    if let Err(error) = app.handle().plugin(plugin) {
                        eprintln!("日志插件初始化失败：{error}");
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    eprintln!("日志配置无法映射到插件：{error}");
                }
            }
            let editor_settings = EditorSettingsService::load(config_dir.join("editor.toml"));
            let project_service = ProjectService::new(data_dir.join("projects.json"));
            let review_service = ReviewService::default();
            app.manage(AppState::new(
                editor_settings,
                project_service,
                review_service,
            ));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::editor_settings::editor_settings_get,
            commands::projects::projects_list,
            commands::projects::projects_add,
            commands::projects::projects_update,
            commands::projects::projects_remove,
            commands::projects::projects_validate_repository,
            commands::projects::projects_list_branches,
            commands::projects::projects_select_directory,
            commands::review::review_get_task,
            commands::review::review_restore_branch_analysis,
            commands::review::review_get_branch_cache_status,
            commands::review::review_set_branch_selected_file,
            commands::review::review_start_analysis,
            commands::review::review_list_changed_files,
            commands::review::review_cancel_analysis,
            commands::review::review_list_authors,
            commands::review::review_get_file_overlay,
            commands::review::review_get_commit_overlay,
            commands::review::review_attribute_blocks
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}

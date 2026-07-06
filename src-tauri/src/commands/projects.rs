use revier_analysis::contracts::{
    DirectorySelection, GitBranch, RepositoryValidation, ReviewProject,
};
use serde::Deserialize;
use std::path::Path;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::error::{command_error, command_error_with_detail, CommandResult};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAddOptions {
    pub name: Option<String>,
}

#[tauri::command]
pub fn projects_list(state: State<'_, AppState>) -> CommandResult<Vec<ReviewProject>> {
    state.projects.list_projects()
}

#[tauri::command]
pub fn projects_add(
    state: State<'_, AppState>,
    repo_path: String,
    options: Option<ProjectAddOptions>,
) -> CommandResult<ReviewProject> {
    let validation = state.projects.validate_repository(&repo_path)?;
    if !validation.valid {
        return Err(command_error_with_detail(
            "REPOSITORY_INVALID",
            validation
                .error
                .unwrap_or_else(|| "请选择一个 Git 仓库目录".to_string()),
            validation.repo_path,
        ));
    }

    state.projects.add_project_with_default_branch(
        validation.repo_path,
        options.and_then(|value| value.name),
        validation.current_branch,
    )
}

#[tauri::command]
pub fn projects_update(
    state: State<'_, AppState>,
    project: ReviewProject,
) -> CommandResult<ReviewProject> {
    state.projects.update_project(project)
}

#[tauri::command]
pub fn projects_remove(state: State<'_, AppState>, project_id: String) -> CommandResult<()> {
    state.projects.remove_project(&project_id)
}

#[tauri::command]
pub fn projects_validate_repository(
    state: State<'_, AppState>,
    repo_path: String,
) -> CommandResult<RepositoryValidation> {
    state.projects.validate_repository(repo_path)
}

#[tauri::command]
pub fn projects_list_branches(
    state: State<'_, AppState>,
    project_id: String,
) -> CommandResult<Vec<GitBranch>> {
    state.projects.list_branches(&project_id)
}

#[tauri::command]
pub async fn projects_select_directory(
    app: AppHandle,
) -> CommandResult<Option<DirectorySelection>> {
    let (sender, mut receiver) = tauri::async_runtime::channel(1);
    app.dialog().file().pick_folder(move |path| {
        let _ = sender.try_send(path);
    });

    let Some(path) = receiver.recv().await else {
        return Err(command_error(
            "DIALOG_RESULT_UNAVAILABLE",
            "目录选择结果未返回",
        ));
    };
    let Some(path) = path else {
        return Ok(None);
    };
    let path = path.into_path().map_err(|error| {
        command_error_with_detail(
            "DIALOG_PATH_INVALID",
            "目录路径无法转换为本地路径",
            error.to_string(),
        )
    })?;
    Ok(Some(directory_selection_from_path(&path)))
}

fn directory_selection_from_path(path: &Path) -> DirectorySelection {
    DirectorySelection {
        path: normalize_selected_directory_path(&path.to_string_lossy()),
        name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("未命名仓库")
            .to_string(),
    }
}

fn normalize_selected_directory_path(path: &str) -> String {
    let normalized = path.replace('\\', "/");
    if normalized == "/"
        || (normalized.len() == 3 && normalized.as_bytes()[1] == b':' && normalized.ends_with('/'))
    {
        normalized
    } else {
        normalized.trim_end_matches('/').to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{directory_selection_from_path, normalize_selected_directory_path};
    use std::path::Path;

    #[test]
    fn normalizes_selected_directory_path_separators_and_trailing_slash() {
        assert_eq!(
            normalize_selected_directory_path("E:\\Projects\\revier\\"),
            "E:/Projects/revier"
        );
    }

    #[test]
    fn keeps_root_directory_paths_readable() {
        assert_eq!(normalize_selected_directory_path("/"), "/");
        assert_eq!(normalize_selected_directory_path("E:/"), "E:/");
    }

    #[test]
    fn derives_directory_selection_name_or_fallback() {
        let selection = directory_selection_from_path(Path::new("revier"));
        assert_eq!(selection.path, "revier");
        assert_eq!(selection.name, "revier");

        let root = directory_selection_from_path(Path::new("/"));
        assert_eq!(root.path, "/");
        assert_eq!(root.name, "未命名仓库");
    }
}

use revier_analysis::contracts::{
    DirectorySelection, GitBranch, RepositoryValidation, ReviewProject,
};
use serde::Deserialize;
use tauri::State;

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
pub fn projects_select_directory() -> CommandResult<Option<DirectorySelection>> {
    Err(command_error(
        "DIALOG_UNAVAILABLE",
        "Tauri dialog 插件尚未接入，当前无法选择目录",
    ))
}

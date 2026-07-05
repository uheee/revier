use revier_analysis::contracts::ReviewProject;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub fn projects_list(state: State<'_, AppState>) -> CommandResult<Vec<ReviewProject>> {
    state.projects.list_projects()
}

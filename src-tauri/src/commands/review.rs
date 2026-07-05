use revier_analysis::contracts::{AnalysisTaskSnapshot, ChangedFile, ReviewFilters};
use tauri::{AppHandle, Emitter, State};

use crate::error::CommandResult;
use crate::state::AppState;

const TASK_UPDATED_EVENT: &str = "review://task-updated";

#[tauri::command]
pub fn review_get_task(
    state: State<'_, AppState>,
    task_id: String,
) -> CommandResult<AnalysisTaskSnapshot> {
    state.review.get_task(&task_id)
}

#[tauri::command]
pub fn review_start_analysis(
    app: AppHandle,
    state: State<'_, AppState>,
    filters: ReviewFilters,
) -> CommandResult<AnalysisTaskSnapshot> {
    let snapshot = state
        .review
        .start_analysis(state.projects.as_ref(), filters)?;
    app.emit(TASK_UPDATED_EVENT, &snapshot)
        .map_err(|error| crate::error::command_error("TASK_EVENT_FAILED", error.to_string()))?;
    Ok(snapshot)
}

#[tauri::command]
pub fn review_list_changed_files(
    state: State<'_, AppState>,
    task_id: String,
) -> CommandResult<Vec<ChangedFile>> {
    state.review.list_changed_files(&task_id)
}

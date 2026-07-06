use revier_analysis::contracts::{
    AnalysisTaskSnapshot, AuthorFilterOption, ChangedFile, CommitOverlayRequest, FileOverlay,
    FileOverlayRequest, ReviewAuthorOptionsRequest, ReviewFilters,
};
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

#[tauri::command]
pub fn review_cancel_analysis(state: State<'_, AppState>, task_id: String) -> CommandResult<()> {
    state.review.cancel_analysis(&task_id)
}

#[tauri::command]
pub fn review_list_authors(
    state: State<'_, AppState>,
    request: ReviewAuthorOptionsRequest,
) -> CommandResult<Vec<AuthorFilterOption>> {
    state.review.list_authors(state.projects.as_ref(), request)
}

#[tauri::command]
pub fn review_get_file_overlay(
    state: State<'_, AppState>,
    request: FileOverlayRequest,
) -> CommandResult<FileOverlay> {
    state.review.get_file_overlay(request)
}

#[tauri::command]
pub fn review_get_commit_overlay(
    state: State<'_, AppState>,
    request: CommitOverlayRequest,
) -> CommandResult<FileOverlay> {
    state.review.get_commit_overlay(request)
}

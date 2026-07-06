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
    let snapshot = state.review.start_analysis_task(filters.clone())?;
    emit_task_update(&app, &snapshot)?;

    let task_id = snapshot.task_id.clone();
    let review = state.review.clone();
    let projects = state.projects.clone();
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = review.execute_analysis_task(projects.as_ref(), &task_id, filters);
        let snapshot = match result {
            Ok(snapshot) => Some(snapshot),
            Err(_) => review.get_task(&task_id).ok(),
        };
        if let Some(snapshot) = snapshot {
            let _ = emit_task_update(&app, &snapshot);
        }
    });

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
pub fn review_cancel_analysis(
    app: AppHandle,
    state: State<'_, AppState>,
    task_id: String,
) -> CommandResult<()> {
    let snapshot = state.review.cancel_analysis(&task_id)?;
    emit_task_update(&app, &snapshot)?;
    Ok(())
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

fn emit_task_update(app: &AppHandle, snapshot: &AnalysisTaskSnapshot) -> CommandResult<()> {
    app.emit(TASK_UPDATED_EVENT, snapshot)
        .map_err(|error| crate::error::command_error("TASK_EVENT_FAILED", error.to_string()))
}

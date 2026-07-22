use revier_analysis::contracts::{
    AnalysisTaskSnapshot, AttributeBlocksRequest, AttributeBlocksResult, AuthorFilterOption,
    BranchAnalysisRestoreResult, BranchCacheStatus, ChangedFile, CommitOverlayRequest, FileOverlay,
    FileOverlayRequest, OperationProgressSnapshot, OperationStage, OperationStatus,
    ReviewAuthorOptionsRequest, ReviewFilters,
};
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::error::CommandResult;
use crate::state::AppState;

const TASK_UPDATED_EVENT: &str = "review://task-updated";
const OPERATION_PROGRESS_EVENT: &str = "review://operation-progress";

#[tauri::command]
pub fn review_restore_branch_analysis(
    state: State<'_, AppState>,
    project_id: String,
    branch: String,
) -> CommandResult<BranchAnalysisRestoreResult> {
    state
        .review
        .restore_branch_analysis(state.projects.as_ref(), &project_id, &branch)
}

#[tauri::command]
pub fn review_get_branch_cache_status(
    state: State<'_, AppState>,
    project_id: String,
    branch: String,
) -> CommandResult<BranchCacheStatus> {
    state
        .review
        .get_branch_cache_status(state.projects.as_ref(), &project_id, &branch)
}

#[tauri::command]
pub fn review_set_branch_selected_file(
    state: State<'_, AppState>,
    project_id: String,
    branch: String,
    file_path: Option<String>,
) -> CommandResult<()> {
    state.review.set_branch_selected_file(
        state.projects.as_ref(),
        &project_id,
        &branch,
        file_path.as_deref(),
    )
}

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
    operation_id: String,
) -> CommandResult<AnalysisTaskSnapshot> {
    let progress_app = app.clone();
    let sink = Arc::new(move |progress: OperationProgressSnapshot| {
        let _ = progress_app.emit(OPERATION_PROGRESS_EVENT, progress);
    });
    let snapshot =
        state
            .review
            .start_analysis_task_with_progress(filters.clone(), operation_id, sink)?;
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
            let status = match &snapshot.status {
                revier_analysis::contracts::AnalysisTaskStatus::Completed => {
                    Some((OperationStatus::Completed, "项目分析完成".to_string()))
                }
                revier_analysis::contracts::AnalysisTaskStatus::Failed => Some((
                    OperationStatus::Failed,
                    snapshot
                        .error
                        .as_ref()
                        .map(|error| error.message.clone())
                        .unwrap_or_else(|| "项目分析失败".to_string()),
                )),
                revier_analysis::contracts::AnalysisTaskStatus::Cancelled => {
                    Some((OperationStatus::Cancelled, "项目分析已取消".to_string()))
                }
                _ => None,
            };
            if let Some((status, message)) = status {
                if let Some(progress) =
                    review.take_operation_terminal_snapshot(&task_id, status, message)
                {
                    let _ = app.emit(OPERATION_PROGRESS_EVENT, progress);
                }
            }
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
    if let Some(progress) = state.review.take_operation_terminal_snapshot(
        &task_id,
        OperationStatus::Cancelled,
        "项目分析已取消".to_string(),
    ) {
        app.emit(OPERATION_PROGRESS_EVENT, progress)
            .map_err(|error| crate::error::command_error("TASK_EVENT_FAILED", error.to_string()))?;
    }
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
    app: AppHandle,
    state: State<'_, AppState>,
    request: FileOverlayRequest,
) -> CommandResult<FileOverlay> {
    let progress_app = app.clone();
    let sink = Arc::new(move |progress: OperationProgressSnapshot| {
        let _ = progress_app.emit(OPERATION_PROGRESS_EVENT, progress);
    });
    state.review.start_file_operation(&request, sink)?;
    let operation_id = request.operation_id.clone();
    let file_path = request.file_path.clone();
    let result = state.review.get_file_overlay(request);
    match &result {
        Ok(_) => {
            state.review.set_file_operation_kind(
                &operation_id,
                revier_analysis::contracts::OperationKind::MonacoDiff,
            );
            state.review.report_file_operation(
                &operation_id,
                OperationStatus::Running,
                OperationStage::ComputeDiff,
                format!("正在计算差异 {file_path}"),
                None,
            )
        }
        Err(error) => state.review.report_file_operation(
            &operation_id,
            OperationStatus::Failed,
            OperationStage::Ready,
            error.message.clone(),
            None,
        ),
    }
    result
}

#[tauri::command]
pub fn review_get_commit_overlay(
    app: AppHandle,
    state: State<'_, AppState>,
    request: CommitOverlayRequest,
) -> CommandResult<FileOverlay> {
    let progress_app = app.clone();
    let sink = Arc::new(move |progress: OperationProgressSnapshot| {
        let _ = progress_app.emit(OPERATION_PROGRESS_EVENT, progress);
    });
    state.review.start_commit_operation(&request, sink)?;
    let operation_id = request.operation_id.clone();
    let commit_hash = request.commit_hash.clone();
    let result = state.review.get_commit_overlay_with_cache(request);
    match &result {
        Ok(output) => state.review.report_file_operation(
            &operation_id,
            OperationStatus::Completed,
            OperationStage::Ready,
            if matches!(
                output.cache_state,
                revier_analysis::contracts::CacheState::Hit
            ) {
                format!("已从缓存加载提交 {commit_hash}")
            } else {
                format!("提交下钻加载完成 {commit_hash}")
            },
            Some(output.cache_state.clone()),
        ),
        Err(error) => state.review.report_file_operation(
            &operation_id,
            OperationStatus::Failed,
            OperationStage::Ready,
            error.message.clone(),
            None,
        ),
    }
    result.map(|output| output.overlay)
}

#[tauri::command]
pub async fn review_attribute_blocks(
    state: State<'_, AppState>,
    request: AttributeBlocksRequest,
) -> CommandResult<AttributeBlocksResult> {
    let operation_id = request.operation_id.clone();
    let review = state.review.clone();
    review.set_file_operation_kind(
        &operation_id,
        revier_analysis::contracts::OperationKind::FileAttribution,
    );
    review.report_file_operation(
        &operation_id,
        OperationStatus::Running,
        OperationStage::AttributeCandidates,
        format!("正在归因 {}", request.file_path),
        None,
    );
    let worker_review = review.clone();
    let result =
        match tauri::async_runtime::spawn_blocking(move || worker_review.attribute_blocks(request))
            .await
        {
            Ok(result) => result,
            Err(error) => {
                let error = crate::error::command_error(
                    "ATTRIBUTE_BLOCKS_JOIN_FAILED",
                    format!("归因任务执行失败：{error}"),
                );
                review.report_file_operation(
                    &operation_id,
                    OperationStatus::Failed,
                    OperationStage::Ready,
                    error.message.clone(),
                    None,
                );
                return Err(error);
            }
        };
    match &result {
        Ok(output) => review.report_file_operation(
            &operation_id,
            OperationStatus::Completed,
            OperationStage::Ready,
            if matches!(
                output.cache_state,
                revier_analysis::contracts::CacheState::Hit
            ) {
                "已从缓存加载文件归因".to_string()
            } else {
                "文件归因完成".to_string()
            },
            Some(output.cache_state.clone()),
        ),
        Err(error) => review.report_file_operation(
            &operation_id,
            OperationStatus::Failed,
            OperationStage::Ready,
            error.message.clone(),
            None,
        ),
    }
    result
}

fn emit_task_update(app: &AppHandle, snapshot: &AnalysisTaskSnapshot) -> CommandResult<()> {
    app.emit(TASK_UPDATED_EVENT, snapshot)
        .map_err(|error| crate::error::command_error("TASK_EVENT_FAILED", error.to_string()))
}

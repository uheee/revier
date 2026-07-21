use std::sync::{Arc, Mutex};

use revier_analysis::contracts::{
    CacheState, OperationKind, OperationProgressSnapshot, OperationStage, OperationStatus,
};
use revier_analysis::execution::{
    AnalysisExecutionContext, OperationProgressReporter, OperationProgressUpdate,
};

#[test]
fn operation_progress_contract_uses_stable_kebab_case_values() {
    assert_eq!(
        serde_json::to_string(&OperationKind::ProjectAnalysis).expect("序列化操作类型"),
        "\"project-analysis\""
    );
    assert_eq!(
        serde_json::to_string(&OperationStatus::Completed).expect("序列化操作状态"),
        "\"completed\""
    );
    assert_eq!(
        serde_json::to_string(&OperationStage::IndexCommits).expect("序列化操作阶段"),
        "\"index-commits\""
    );
    assert_eq!(
        serde_json::to_string(&CacheState::Hit).expect("序列化缓存状态"),
        "\"hit\""
    );
}

#[test]
fn operation_progress_omits_unknown_scope_and_progress() {
    let snapshot = OperationProgressSnapshot {
        operation_id: "operation-1".to_string(),
        kind: OperationKind::ProjectAnalysis,
        status: OperationStatus::Running,
        project_id: "project-1".to_string(),
        branch: Some("main".to_string()),
        file_path: None,
        commit_hash: None,
        stage: OperationStage::ReadRepository,
        message: "读取仓库".to_string(),
        completed_units: None,
        total_units: None,
        progress: None,
        started_at: "2026-07-21T00:00:00Z".to_string(),
        elapsed_ms: 12,
        cache_state: CacheState::None,
    };

    let value = serde_json::to_value(snapshot).expect("序列化操作进度");
    assert_eq!(value["operationId"], "operation-1");
    assert_eq!(value["elapsedMs"], 12);
    assert!(value.get("filePath").is_none());
    assert!(value.get("commitHash").is_none());
    assert!(value.get("completedUnits").is_none());
    assert!(value.get("totalUnits").is_none());
    assert!(value.get("progress").is_none());
}

#[derive(Default)]
struct RecordingReporter {
    updates: Mutex<Vec<OperationProgressUpdate>>,
}

impl OperationProgressReporter for RecordingReporter {
    fn report(&self, update: OperationProgressUpdate) {
        self.updates.lock().expect("进度记录锁被污染").push(update);
    }
}

#[test]
fn execution_context_forwards_progress_without_tauri_dependency() {
    let reporter = Arc::new(RecordingReporter::default());
    let context = AnalysisExecutionContext::with_progress(reporter.clone());
    let update = OperationProgressUpdate {
        stage: OperationStage::IndexCommits,
        message: "索引提交".to_string(),
        completed_units: Some(2),
        total_units: Some(4),
    };

    context.report_progress(update.clone());

    let updates = reporter.updates.lock().expect("进度记录锁被污染");
    assert_eq!(updates.as_slice(), &[update]);
    assert!(!context.is_cancelled());
}

#[test]
fn execution_context_combines_cancellation_and_progress() {
    let reporter = Arc::new(RecordingReporter::default());
    let context = AnalysisExecutionContext::with_cancel_and_progress(|| true, reporter.clone());

    context.report_progress(OperationProgressUpdate {
        stage: OperationStage::Ready,
        message: "完成".to_string(),
        completed_units: Some(1),
        total_units: Some(1),
    });

    assert!(context.is_cancelled());
    assert_eq!(reporter.updates.lock().expect("进度记录锁被污染").len(), 1);
}

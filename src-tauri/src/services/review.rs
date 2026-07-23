use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use chrono::{DateTime, Utc};
use revier_analysis::api::QueryFilesRequest;
use revier_analysis::cache::models::{
    CachedAnalysisFile, CachedAnalysisSnapshot, CachedBlockCommit, CachedCommitOverlay,
    CachedCommitOverlayBlock, CachedFileAnalysis, CachedFileBlock, CachedTouchedRange,
};
use revier_analysis::cli::{IndexBuildArgs, IndexCommonArgs, OutputFormat, OverlayCommonArgs};
use revier_analysis::contracts::{
    AnalysisRange, AnalysisStage, AnalysisTaskSnapshot, AnalysisTaskStatus, AttributeBlocksRequest,
    AttributeBlocksResult, AttributionConfidence, AttributionMethod, AttributionWarning,
    AttributionWarningCode, AuthorFilterOption, AuthorSummary, BlockAttributionSummary,
    BranchAnalysisRestoreResult, BranchCacheStatus, CacheMode, CacheState, ChangedFile,
    ChangedFileStatus, CommitOverlayRequest, DiffBlock, DiffBlockAttribution, DiffBlockChangeType,
    FileOverlay, FileOverlayMode, FileOverlayRequest, OperationKind, OperationProgressSnapshot,
    OperationStage, OperationStatus, ProjectId, RelatedCommit, RelatedCommitAttribution,
    ResolvedTextEncoding, ReviewAuthorOptionsRequest, ReviewFilters, ReviewProject,
    SideBySideDiffRow, SideBySideDiffRowType, TaskId, TextEncoding, TouchedRange, WordChange,
};
use revier_analysis::error::AppError as AnalysisAppError;
use revier_analysis::execution::{
    AnalysisExecutionContext, OperationProgressReporter, OperationProgressUpdate,
};
use revier_analysis::index::connection::DatabaseRegistry;
use revier_analysis::json::{
    AttributionWarningOutput, AuthorOutput, BlockAttributionOutput, ChangedFileOutput,
    DiffBlockOutput, RelatedCommitAttributionOutput, RelatedCommitOutput, SideBySideDiffRowOutput,
    TouchedRangeOutput, WordChangeOutput,
};
use tokio_util::sync::CancellationToken;

use crate::error::{command_error, command_error_with_detail, CommandResult};
use crate::services::projects::ProjectService;

#[derive(Default)]
pub struct ReviewService {
    database_registry: Arc<DatabaseRegistry>,
    tasks: Arc<Mutex<HashMap<TaskId, AnalysisTaskSnapshot>>>,
    filters_by_task: Mutex<HashMap<TaskId, ReviewFilters>>,
    files_by_task: Mutex<HashMap<TaskId, Vec<ChangedFile>>>,
    contexts_by_task: Mutex<HashMap<TaskId, ReviewTaskContext>>,
    cancellations_by_task: Mutex<HashMap<TaskId, CancellationToken>>,
    progress_by_task: Mutex<HashMap<TaskId, TaskProgressRegistration>>,
    progress_by_file_operation: Mutex<HashMap<String, FileProgressRegistration>>,
}

pub(crate) type ProgressEventSink = Arc<dyn Fn(OperationProgressSnapshot) + Send + Sync>;

#[derive(Clone)]
struct TaskProgressRegistration {
    operation_id: String,
    project_id: String,
    branch: String,
    started_at: String,
    started: Instant,
    sink: ProgressEventSink,
}

#[derive(Clone)]
struct FileProgressRegistration {
    operation_id: String,
    project_id: String,
    branch: String,
    file_path: String,
    kind: OperationKind,
    commit_hash: Option<String>,
    started_at: String,
    started: Instant,
    cache_state: CacheState,
    sink: ProgressEventSink,
}

#[derive(Clone)]
struct ReviewTaskContext {
    project: ReviewProject,
    filters: ReviewFilters,
    range: AnalysisRange,
}

struct AnalysisRunResult {
    files: Vec<ChangedFile>,
    context: ReviewTaskContext,
}

pub struct CommitOverlayLoadResult {
    pub overlay: FileOverlay,
    pub cache_state: CacheState,
}

struct TaskProgressReporter {
    tasks: Arc<Mutex<HashMap<TaskId, AnalysisTaskSnapshot>>>,
    task_id: TaskId,
    started: Instant,
    registration: Option<TaskProgressRegistration>,
    emission: Mutex<ProgressEmissionState>,
}

#[derive(Default)]
struct ProgressEmissionState {
    last_stage: Option<OperationStage>,
    last_emitted_at: Option<Instant>,
}

impl OperationProgressReporter for TaskProgressReporter {
    fn report(&self, update: OperationProgressUpdate) {
        #[cfg(debug_assertions)]
        if std::env::var_os("REVIER_TRACE_OPERATIONS").is_some() {
            eprintln!(
                "分析进度 task={} stage={:?} completed={:?} total={:?} elapsed_ms={}",
                self.task_id,
                update.stage,
                update.completed_units,
                update.total_units,
                self.started.elapsed().as_millis()
            );
        }
        let progress = match (update.completed_units, update.total_units) {
            (Some(completed), Some(total)) if total > 0 => {
                Some((completed as f64 / total as f64).clamp(0.0, 1.0))
            }
            (Some(0), Some(0)) => Some(1.0),
            _ => None,
        };
        {
            let mut tasks = self.tasks.lock().expect("任务锁被污染");
            let Some(task) = tasks.get_mut(&self.task_id) else {
                return;
            };
            if matches!(task.status, AnalysisTaskStatus::Cancelled) {
                return;
            }
            task.status = AnalysisTaskStatus::Running;
            task.stage = analysis_stage_for_operation(&update.stage);
            task.progress = progress;
            task.message = Some(update.message.clone());
            task.error = None;
        }

        let Some(registration) = self.registration.as_ref() else {
            return;
        };
        let now = Instant::now();
        let mut emission = self.emission.lock().expect("进度事件锁被污染");
        let stage_changed = emission.last_stage.as_ref() != Some(&update.stage);
        let interval_reached = emission
            .last_emitted_at
            .is_none_or(|last| now.duration_since(last).as_millis() >= 100);
        if !stage_changed && !interval_reached {
            return;
        }
        emission.last_stage = Some(update.stage.clone());
        emission.last_emitted_at = Some(now);
        drop(emission);
        (registration.sink)(OperationProgressSnapshot {
            operation_id: registration.operation_id.clone(),
            kind: OperationKind::ProjectAnalysis,
            status: OperationStatus::Running,
            project_id: registration.project_id.clone(),
            branch: Some(registration.branch.clone()),
            file_path: None,
            commit_hash: None,
            stage: update.stage,
            message: update.message,
            completed_units: update.completed_units,
            total_units: update.total_units,
            progress,
            started_at: registration.started_at.clone(),
            elapsed_ms: registration.started.elapsed().as_millis() as u64,
            cache_state: CacheState::Refresh,
        });
    }
}

fn analysis_stage_for_operation(stage: &OperationStage) -> AnalysisStage {
    match stage {
        OperationStage::ReadRepository => AnalysisStage::ReadRepository,
        OperationStage::ResolveRange => AnalysisStage::ResolveRange,
        OperationStage::IndexCommits => AnalysisStage::LoadCommits,
        OperationStage::Ready => AnalysisStage::Ready,
        _ => AnalysisStage::LoadChangedFiles,
    }
}

impl ReviewService {
    pub fn start_file_operation(
        &self,
        request: &FileOverlayRequest,
        sink: ProgressEventSink,
    ) -> CommandResult<()> {
        self.ensure_completed_task(&request.task_id)?;
        let context = self.context_for_task(&request.task_id)?;
        self.file_for_task(&request.task_id, &request.file_path)?;
        let registration = FileProgressRegistration {
            operation_id: request.operation_id.clone(),
            project_id: context.project.id,
            branch: context.range.branch,
            file_path: request.file_path.clone(),
            kind: OperationKind::FileOverlay,
            commit_hash: None,
            started_at: Utc::now().to_rfc3339(),
            started: Instant::now(),
            cache_state: match request.cache_mode {
                CacheMode::PreferCache => CacheState::None,
                CacheMode::Refresh => CacheState::Refresh,
            },
            sink,
        };
        let mut operations = self
            .progress_by_file_operation
            .lock()
            .expect("文件进度注册锁被污染");
        operations.retain(|_, item| item.started.elapsed().as_secs() < 3600);
        if operations.len() >= 128 {
            if let Some(oldest) = operations
                .iter()
                .max_by_key(|(_, item)| item.started.elapsed())
                .map(|(key, _)| key.clone())
            {
                operations.remove(&oldest);
            }
        }
        operations.insert(request.operation_id.clone(), registration);
        drop(operations);
        self.report_file_operation(
            &request.operation_id,
            OperationStatus::Running,
            OperationStage::ReadFileContent,
            format!("正在读取 {}", request.file_path),
            None,
        );
        Ok(())
    }

    pub fn start_commit_operation(
        &self,
        request: &CommitOverlayRequest,
        sink: ProgressEventSink,
    ) -> CommandResult<()> {
        self.ensure_completed_task(&request.task_id)?;
        let context = self.context_for_task(&request.task_id)?;
        self.file_for_task(&request.task_id, &request.file_path)?;
        let registration = FileProgressRegistration {
            operation_id: request.operation_id.clone(),
            project_id: context.project.id,
            branch: context.range.branch,
            file_path: request.file_path.clone(),
            kind: OperationKind::CommitOverlay,
            commit_hash: Some(request.commit_hash.clone()),
            started_at: Utc::now().to_rfc3339(),
            started: Instant::now(),
            cache_state: match request.cache_mode {
                CacheMode::PreferCache => CacheState::None,
                CacheMode::Refresh => CacheState::Refresh,
            },
            sink,
        };
        let mut operations = self
            .progress_by_file_operation
            .lock()
            .expect("文件进度注册锁被污染");
        operations.retain(|_, item| item.started.elapsed().as_secs() < 3600);
        if operations.len() >= 128 {
            if let Some(oldest) = operations
                .iter()
                .max_by_key(|(_, item)| item.started.elapsed())
                .map(|(key, _)| key.clone())
            {
                operations.remove(&oldest);
            }
        }
        operations.insert(request.operation_id.clone(), registration);
        drop(operations);
        self.report_file_operation(
            &request.operation_id,
            OperationStatus::Running,
            OperationStage::ReadFileContent,
            format!("正在打开提交 {}", request.commit_hash),
            None,
        );
        Ok(())
    }

    pub fn report_file_operation(
        &self,
        operation_id: &str,
        status: OperationStatus,
        stage: OperationStage,
        message: String,
        cache_state: Option<CacheState>,
    ) {
        let terminal = !matches!(status, OperationStatus::Running);
        let registration = {
            let mut operations = self
                .progress_by_file_operation
                .lock()
                .expect("文件进度注册锁被污染");
            if terminal {
                operations.remove(operation_id)
            } else {
                operations.get(operation_id).cloned()
            }
        };
        let Some(registration) = registration else {
            return;
        };
        let elapsed_ms = registration.started.elapsed().as_millis() as u64;
        let progress = matches!(status, OperationStatus::Completed).then_some(1.0);
        (registration.sink)(OperationProgressSnapshot {
            operation_id: registration.operation_id,
            kind: registration.kind,
            status,
            project_id: registration.project_id,
            branch: Some(registration.branch),
            file_path: Some(registration.file_path),
            commit_hash: registration.commit_hash,
            stage,
            message,
            completed_units: None,
            total_units: None,
            progress,
            started_at: registration.started_at,
            elapsed_ms,
            cache_state: cache_state.unwrap_or(registration.cache_state),
        });
    }

    pub fn set_file_operation_kind(&self, operation_id: &str, kind: OperationKind) {
        if let Some(registration) = self
            .progress_by_file_operation
            .lock()
            .expect("文件进度注册锁被污染")
            .get_mut(operation_id)
        {
            registration.kind = kind;
        }
    }

    pub fn get_branch_cache_status(
        &self,
        projects: &ProjectService,
        project_id: &str,
        branch: &str,
    ) -> CommandResult<BranchCacheStatus> {
        let started = Instant::now();
        let project = projects.get_project(project_id)?;
        let (repo_id, current_head, db_path) = branch_cache_location(&project, branch)?;
        let (cache_state, cached_head) = if db_path.exists() {
            let conn = self
                .database_registry
                .connect(&db_path)
                .map_err(map_analysis_error)?;
            revier_analysis::index::migrations::ensure_compatible_schema(&conn)
                .map_err(map_analysis_error)?;
            let snapshot =
                revier_analysis::cache::repository::load_branch_snapshot(&conn, &repo_id, branch)
                    .map_err(map_analysis_error)?;
            match snapshot {
                Some(snapshot) => {
                    let state = if snapshot.head_commit == current_head
                        && snapshot.analysis_version == revier_analysis::cache::ANALYSIS_VERSION
                    {
                        CacheState::Hit
                    } else {
                        CacheState::Stale
                    };
                    (state, Some(snapshot.head_commit))
                }
                None => (CacheState::Miss, None),
            }
        } else {
            (CacheState::Miss, None)
        };

        Ok(BranchCacheStatus {
            project_id: project_id.to_string(),
            branch: branch.to_string(),
            cache_state,
            current_head,
            cached_head,
            cache_read_elapsed_ms: started.elapsed().as_millis() as u64,
        })
    }

    pub fn restore_branch_analysis(
        &self,
        projects: &ProjectService,
        project_id: &str,
        branch: &str,
    ) -> CommandResult<BranchAnalysisRestoreResult> {
        let started = Instant::now();
        let project = projects.get_project(project_id)?;
        let (repo_id, current_head, db_path) = branch_cache_location(&project, branch)?;
        if !db_path.exists() {
            return Ok(empty_branch_restore(
                project_id,
                branch,
                current_head,
                started.elapsed().as_millis() as u64,
            ));
        }
        let conn = self
            .database_registry
            .connect(&db_path)
            .map_err(map_analysis_error)?;
        revier_analysis::index::migrations::ensure_compatible_schema(&conn)
            .map_err(map_analysis_error)?;
        let Some(snapshot) =
            revier_analysis::cache::repository::load_branch_snapshot(&conn, &repo_id, branch)
                .map_err(map_analysis_error)?
        else {
            return Ok(empty_branch_restore(
                project_id,
                branch,
                current_head,
                started.elapsed().as_millis() as u64,
            ));
        };

        let stale = snapshot.head_commit != current_head
            || snapshot.analysis_version != revier_analysis::cache::ANALYSIS_VERSION;
        let files = snapshot
            .files
            .iter()
            .map(adapt_cached_analysis_file)
            .collect::<CommandResult<Vec<_>>>()?;
        let filters = ReviewFilters {
            project_id: project_id.to_string(),
            branch: snapshot.branch.clone(),
            start_at: snapshot.start_at.clone(),
            end_at: snapshot.end_at.clone(),
            author_keys: Some(snapshot.author_keys.clone()),
            author_query: snapshot.author_query.clone(),
            message_query: snapshot.message_query.clone(),
            glob_rules: snapshot.globs.clone(),
        };
        let range = AnalysisRange {
            branch: snapshot.branch.clone(),
            base_commit: snapshot.base_commit.clone(),
            head_commit: snapshot.head_commit.clone(),
            start_at: snapshot.start_at.clone(),
            end_at: snapshot.end_at.clone(),
        };
        let task = self.create_task(project_id.to_string());
        self.files_by_task
            .lock()
            .expect("文件缓存锁被污染")
            .insert(task.task_id.clone(), files.clone());
        self.filters_by_task
            .lock()
            .expect("筛选条件锁被污染")
            .insert(task.task_id.clone(), filters.clone());
        self.contexts_by_task
            .lock()
            .expect("任务上下文锁被污染")
            .insert(
                task.task_id.clone(),
                ReviewTaskContext {
                    project,
                    filters: filters.clone(),
                    range: range.clone(),
                },
            );
        self.mark_completed(&task.task_id);
        let task = self.get_task(&task.task_id)?;
        let last_selected_path = snapshot
            .last_selected_path
            .filter(|path| files.iter().any(|file| file.path == *path));

        Ok(BranchAnalysisRestoreResult {
            project_id: project_id.to_string(),
            branch: branch.to_string(),
            cache_state: if stale {
                CacheState::Stale
            } else {
                CacheState::Hit
            },
            cache_hit: true,
            stale,
            task: Some(task),
            range: Some(range),
            filters: Some(filters),
            files,
            last_selected_path,
            current_head,
            cached_head: Some(snapshot.head_commit),
            cache_read_elapsed_ms: started.elapsed().as_millis() as u64,
            analysis_elapsed_ms: Some(snapshot.elapsed_ms),
        })
    }

    pub fn set_branch_selected_file(
        &self,
        projects: &ProjectService,
        project_id: &str,
        branch: &str,
        file_path: Option<&str>,
    ) -> CommandResult<()> {
        let project = projects.get_project(project_id)?;
        let (repo_id, _, db_path) = branch_cache_location(&project, branch)?;
        if !db_path.exists() {
            return Err(command_error(
                "BRANCH_CACHE_NOT_FOUND",
                format!("分支缓存不存在：{branch}"),
            ));
        }
        let conn = self
            .database_registry
            .connect(&db_path)
            .map_err(map_analysis_error)?;
        revier_analysis::index::migrations::ensure_compatible_schema(&conn)
            .map_err(map_analysis_error)?;
        revier_analysis::cache::repository::update_last_selected_path(
            &conn, &repo_id, branch, file_path,
        )
        .map_err(map_analysis_error)
    }

    pub fn create_task(&self, project_id: ProjectId) -> AnalysisTaskSnapshot {
        let task = AnalysisTaskSnapshot {
            task_id: uuid::Uuid::new_v4().to_string(),
            project_id,
            status: AnalysisTaskStatus::Pending,
            stage: AnalysisStage::ReadRepository,
            progress: None,
            message: None,
            error: None,
        };
        self.tasks
            .lock()
            .expect("任务锁被污染")
            .insert(task.task_id.clone(), task.clone());
        task
    }

    pub fn get_task(&self, task_id: &str) -> CommandResult<AnalysisTaskSnapshot> {
        self.tasks
            .lock()
            .expect("任务锁被污染")
            .get(task_id)
            .cloned()
            .ok_or_else(|| command_error("TASK_NOT_FOUND", format!("未找到任务：{task_id}")))
    }

    pub fn mark_running(&self, task_id: &str, stage: AnalysisStage, message: &str) {
        if let Some(task) = self.tasks.lock().expect("任务锁被污染").get_mut(task_id) {
            if matches!(task.status, AnalysisTaskStatus::Cancelled) {
                return;
            }
            task.status = AnalysisTaskStatus::Running;
            task.stage = stage;
            task.progress = None;
            task.message = Some(message.to_string());
            task.error = None;
        }
    }

    pub fn mark_completed(&self, task_id: &str) {
        if let Some(task) = self.tasks.lock().expect("任务锁被污染").get_mut(task_id) {
            if matches!(task.status, AnalysisTaskStatus::Cancelled) {
                self.clear_task_results(task_id);
                self.cleanup_cancellation(task_id);
                return;
            }
            task.status = AnalysisTaskStatus::Completed;
            task.stage = AnalysisStage::Ready;
            task.progress = Some(1.0);
            task.message = None;
            task.error = None;
        }
    }

    #[cfg(test)]
    fn start_analysis(
        &self,
        projects: &ProjectService,
        filters: ReviewFilters,
    ) -> CommandResult<AnalysisTaskSnapshot> {
        let task = self.start_analysis_task(filters.clone())?;
        self.execute_analysis_task(projects, &task.task_id, filters)
    }

    pub fn start_analysis_task(
        &self,
        filters: ReviewFilters,
    ) -> CommandResult<AnalysisTaskSnapshot> {
        let task = self.create_task(filters.project_id.clone());
        self.filters_by_task
            .lock()
            .expect("筛选条件锁被污染")
            .insert(task.task_id.clone(), filters.clone());
        self.cancellations_by_task
            .lock()
            .expect("取消令牌锁被污染")
            .insert(task.task_id.clone(), CancellationToken::new());
        self.mark_running(&task.task_id, AnalysisStage::ReadRepository, "等待后台分析");
        self.get_task(&task.task_id)
    }

    pub fn start_analysis_task_with_progress(
        &self,
        filters: ReviewFilters,
        operation_id: String,
        sink: ProgressEventSink,
    ) -> CommandResult<AnalysisTaskSnapshot> {
        let task = self.start_analysis_task(filters.clone())?;
        self.progress_by_task
            .lock()
            .expect("进度注册锁被污染")
            .insert(
                task.task_id.clone(),
                TaskProgressRegistration {
                    operation_id,
                    project_id: filters.project_id,
                    branch: filters.branch,
                    started_at: Utc::now().to_rfc3339(),
                    started: Instant::now(),
                    sink,
                },
            );
        Ok(task)
    }

    pub fn take_operation_terminal_snapshot(
        &self,
        task_id: &str,
        status: OperationStatus,
        message: String,
    ) -> Option<OperationProgressSnapshot> {
        let registration = self
            .progress_by_task
            .lock()
            .expect("进度注册锁被污染")
            .remove(task_id)?;
        let progress = matches!(status, OperationStatus::Completed).then_some(1.0);
        Some(OperationProgressSnapshot {
            operation_id: registration.operation_id,
            kind: OperationKind::ProjectAnalysis,
            status,
            project_id: registration.project_id,
            branch: Some(registration.branch),
            file_path: None,
            commit_hash: None,
            stage: OperationStage::Ready,
            message,
            completed_units: None,
            total_units: None,
            progress,
            started_at: registration.started_at,
            elapsed_ms: registration.started.elapsed().as_millis() as u64,
            cache_state: CacheState::Refresh,
        })
    }

    pub fn execute_analysis_task(
        &self,
        projects: &ProjectService,
        task_id: &str,
        filters: ReviewFilters,
    ) -> CommandResult<AnalysisTaskSnapshot> {
        match self.run_analysis(projects, task_id, filters) {
            Ok(result) => {
                if self.is_cancelled(task_id) {
                    self.cleanup_cancellation(task_id);
                    return self.get_task(task_id);
                }
                self.files_by_task
                    .lock()
                    .expect("文件缓存锁被污染")
                    .insert(task_id.to_string(), result.files);
                self.contexts_by_task
                    .lock()
                    .expect("任务上下文锁被污染")
                    .insert(task_id.to_string(), result.context);
                self.mark_completed(task_id);
                self.cleanup_cancellation(task_id);
                self.get_task(task_id)
            }
            Err(error) => {
                if self.is_cancelled(task_id) || error.code == "TASK_CANCELLED" {
                    self.cleanup_cancellation(task_id);
                    return self.get_task(task_id);
                }
                self.mark_failed(task_id, error.clone());
                self.cleanup_cancellation(task_id);
                Err(error)
            }
        }
    }

    pub fn list_changed_files(&self, task_id: &str) -> CommandResult<Vec<ChangedFile>> {
        self.ensure_completed_task(task_id)?;

        self.files_by_task
            .lock()
            .expect("文件缓存锁被污染")
            .get(task_id)
            .cloned()
            .ok_or_else(|| {
                command_error(
                    "TASK_FILES_NOT_FOUND",
                    format!("任务文件缓存不存在：{task_id}"),
                )
            })
    }

    pub fn cancel_analysis(&self, task_id: &str) -> CommandResult<AnalysisTaskSnapshot> {
        if let Some(token) = self
            .cancellations_by_task
            .lock()
            .expect("取消令牌锁被污染")
            .get(task_id)
            .cloned()
        {
            token.cancel();
        }

        let snapshot = {
            let mut tasks = self.tasks.lock().expect("任务锁被污染");
            let task = tasks
                .get_mut(task_id)
                .ok_or_else(|| command_error("TASK_NOT_FOUND", format!("未找到任务：{task_id}")))?;
            if matches!(
                task.status,
                AnalysisTaskStatus::Pending
                    | AnalysisTaskStatus::Running
                    | AnalysisTaskStatus::Cancelled
            ) {
                task.status = AnalysisTaskStatus::Cancelled;
                task.stage = AnalysisStage::Ready;
                task.progress = None;
                task.message = Some("任务已取消".to_string());
                task.error = None;
            }
            task.clone()
        };

        if matches!(snapshot.status, AnalysisTaskStatus::Cancelled) {
            self.clear_task_results(task_id);
        }

        Ok(snapshot)
    }

    pub fn list_authors(
        &self,
        projects: &ProjectService,
        request: ReviewAuthorOptionsRequest,
    ) -> CommandResult<Vec<AuthorFilterOption>> {
        let project = projects.get_project(&request.project_id)?;
        let repo = revier_analysis::git::repository::open_repository(Path::new(&project.repo_path))
            .map_err(map_analysis_error)?;
        let start_at = parse_optional_time(request.start_at.as_deref(), "start_at")?;
        let end_at = parse_optional_time(request.end_at.as_deref(), "end_at")?;
        let commits = revier_analysis::git::commits::list_reachable_commits(&repo, &request.branch)
            .map_err(map_analysis_error)?;
        let mut authors: HashMap<String, AuthorFilterOption> = HashMap::new();

        for commit in commits {
            let committed_at = parse_commit_time(&commit)?;
            if start_at.is_some_and(|start| committed_at < start)
                || end_at.is_some_and(|end| committed_at > end)
            {
                continue;
            }

            let entry =
                authors
                    .entry(commit.author_key.clone())
                    .or_insert_with(|| AuthorFilterOption {
                        key: commit.author_key.clone(),
                        name: commit.author_name.clone(),
                        email: commit.author_email.clone(),
                        commit_count: 0,
                    });
            entry.name = commit.author_name;
            entry.email = commit.author_email;
            entry.commit_count += 1;
        }

        let mut authors = authors.into_values().collect::<Vec<_>>();
        authors.sort_by(|left, right| {
            right
                .commit_count
                .cmp(&left.commit_count)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(authors)
    }

    pub fn get_file_overlay(&self, request: FileOverlayRequest) -> CommandResult<FileOverlay> {
        self.ensure_completed_task(&request.task_id)?;
        let context = self.context_for_task(&request.task_id)?;
        ensure_file_refresh_is_current(&context, request.cache_mode)?;
        let file = self.file_for_task(&request.task_id, &request.file_path)?;
        let document = load_range_overlay_document(
            &context,
            &request.file_path,
            request.encoding.unwrap_or(TextEncoding::Auto),
            &self.database_registry,
        )?;

        Ok(FileOverlay {
            mode: Some(FileOverlayMode::Range),
            file,
            range: context.range,
            rows: None,
            blocks: Vec::new(),
            warnings: document.warnings,
            commit: None,
            parent_hash: None,
            old_content: document.old_text,
            new_content: document.new_text,
            resolved_encoding: document.resolved_encoding,
        })
    }

    pub fn attribute_blocks(
        &self,
        request: AttributeBlocksRequest,
    ) -> CommandResult<AttributeBlocksResult> {
        self.ensure_completed_task(&request.task_id)?;
        let context = self.context_for_task(&request.task_id)?;
        ensure_file_refresh_is_current(&context, request.cache_mode)?;
        self.file_for_task(&request.task_id, &request.file_path)?;
        let content_started = Instant::now();
        let document = load_range_overlay_document(
            &context,
            &request.file_path,
            resolved_encoding_as_requested(request.resolved_encoding),
            &self.database_registry,
        )?;
        let content_elapsed_ms = content_started.elapsed().as_millis() as u64;
        if document.resolved_encoding != request.resolved_encoding {
            return Err(command_error_with_detail(
                "ENCODING_CHANGED",
                "文件编码解析结果已变化，无法合并归因",
                format!(
                    "request={:?}; actual={:?}",
                    request.resolved_encoding, document.resolved_encoding
                ),
            ));
        }

        let (_, conn) = cache_connection_for_context(&context, &self.database_registry)?;
        if matches!(request.cache_mode, CacheMode::PreferCache) {
            if let Some(cached) = revier_analysis::cache::repository::load_file_analysis(
                &conn,
                &document.analysis_id,
                &request.file_path,
            )
            .map_err(map_analysis_error)?
            .filter(|cached| {
                cached.analysis_version == revier_analysis::cache::ANALYSIS_VERSION
                    && cached.resolved_encoding == resolved_encoding_code(request.resolved_encoding)
                    && cached.block_signature == request.block_signature
            }) {
                if let Some(attributions) =
                    restore_cached_attributions(&conn, &cached, &request.blocks)?
                {
                    return Ok(AttributeBlocksResult {
                        resolved_encoding: request.resolved_encoding,
                        attributions,
                        warnings: document.warnings,
                        cache_state: CacheState::Hit,
                    });
                }
            }
        }
        drop(conn);

        let attribution_started = Instant::now();
        let repo = revier_analysis::git::repository::open_repository(Path::new(
            &context.project.repo_path,
        ))
        .map_err(map_analysis_error)?;
        let (_, attribution_connection) =
            cache_connection_for_context(&context, &self.database_registry)?;
        let attribution_context =
            revier_analysis::attribution::context::AttributionContext::open_with_connection(
                &repo,
                &overlay_common_args(&context),
                attribution_connection,
            )
            .map_err(map_analysis_error)?;
        let mut warnings = document.warnings;
        for warning in attribution_context.warnings.iter() {
            append_app_warning(&mut warnings, warning);
        }

        let path_candidates = revier_analysis::attribution::path_history::path_candidates(
            &attribution_context,
            &context.range.base_commit,
            &context.range.head_commit,
            &document.change.path,
            document.change.old_path.as_deref(),
        )
        .map_err(map_analysis_error)?;
        for warning in path_candidates.warnings {
            append_app_warning(&mut warnings, &warning);
        }

        let blocks = revier_analysis::overlay::block_ranges::build_blocks_from_ranges(
            &document.old_text,
            &document.new_text,
            &request.blocks,
        )
        .map_err(map_analysis_error)?;
        let attribution_options =
            revier_analysis::attribution::patch_inference::AttributionOptions {
                encoding: document.resolved_encoding,
                authors: &context.filters.author_keys.clone().unwrap_or_default(),
                author_query: context.filters.author_query.as_deref(),
                message: context.filters.message_query.as_deref(),
            };
        let blocks = revier_analysis::attribution::patch_inference::attach_patch_inference(
            &attribution_context,
            blocks,
            &document.change.path,
            document.change.old_path.as_deref(),
            &attribution_options,
        )
        .map_err(map_analysis_error)?;
        let blocks = revier_analysis::attribution::blame::attach_blame_attribution(
            &attribution_context,
            &context.range.head_commit,
            &document.change.path,
            blocks,
            &attribution_options,
        )
        .map_err(map_analysis_error)?;
        let blocks = revier_analysis::attribution::deletion_trace::attach_deletion_trace(
            &attribution_context,
            blocks,
            &path_candidates.paths,
            &document.change.path,
            document.change.old_path.as_deref(),
            &attribution_options,
        )
        .map_err(map_analysis_error)?;
        drop(attribution_context);

        let cached = cached_file_analysis(
            &document.analysis_id,
            &request.file_path,
            request.resolved_encoding,
            &request.block_signature,
            content_elapsed_ms,
            attribution_started.elapsed().as_millis() as u64,
            &blocks,
        )?;
        let (_, conn) = cache_connection_for_context(&context, &self.database_registry)?;
        revier_analysis::cache::repository::replace_file_analysis(&conn, &cached)
            .map_err(map_analysis_error)?;
        Ok(AttributeBlocksResult {
            resolved_encoding: document.resolved_encoding,
            attributions: adapt_block_attributions(blocks)?,
            warnings,
            cache_state: if matches!(request.cache_mode, CacheMode::Refresh) {
                CacheState::Refresh
            } else {
                CacheState::Miss
            },
        })
    }

    #[cfg(test)]
    fn get_commit_overlay(&self, request: CommitOverlayRequest) -> CommandResult<FileOverlay> {
        self.get_commit_overlay_with_cache(request)
            .map(|result| result.overlay)
    }

    pub fn get_commit_overlay_with_cache(
        &self,
        request: CommitOverlayRequest,
    ) -> CommandResult<CommitOverlayLoadResult> {
        let started = Instant::now();
        self.ensure_completed_task(&request.task_id)?;
        let context = self.context_for_task(&request.task_id)?;
        let file = self.file_for_task(&request.task_id, &request.file_path)?;
        let cacheable_task = self
            .filters_by_task
            .lock()
            .expect("筛选条件锁被污染")
            .contains_key(&request.task_id);
        let mut file_analysis = None;
        if cacheable_task {
            let (repo_id, conn) = cache_connection_for_context(&context, &self.database_registry)?;
            let analysis_file = revier_analysis::cache::repository::load_branch_analysis_file(
                &conn,
                &repo_id,
                &context.range.branch,
                &file.path,
            )
            .map_err(map_analysis_error)?;
            file_analysis = match analysis_file {
                Some((analysis_id, _)) => revier_analysis::cache::repository::load_file_analysis(
                    &conn,
                    &analysis_id,
                    &file.path,
                )
                .map_err(map_analysis_error)?,
                None => None,
            }
            .filter(|cached| cached.analysis_version == revier_analysis::cache::ANALYSIS_VERSION);
            if matches!(request.cache_mode, CacheMode::PreferCache) {
                if let Some(file_analysis) = file_analysis.as_ref() {
                    if let Some(cached) = revier_analysis::cache::repository::load_commit_overlay(
                        &conn,
                        &file_analysis.file_analysis_id,
                        &request.commit_hash,
                    )
                    .map_err(map_analysis_error)?
                    .filter(|cached| {
                        cached.analysis_version == revier_analysis::cache::ANALYSIS_VERSION
                    }) {
                        if let Some(overlay) = restore_cached_commit_overlay(
                            &context,
                            &file,
                            &cached,
                            request.encoding.unwrap_or(TextEncoding::Auto),
                        )? {
                            return Ok(CommitOverlayLoadResult {
                                overlay,
                                cache_state: CacheState::Hit,
                            });
                        }
                    }
                }
            }
        }
        let repo = revier_analysis::git::repository::open_repository(Path::new(
            &context.project.repo_path,
        ))
        .map_err(map_analysis_error)?;
        let commit_reachable = revier_analysis::git::commits::is_commit_reachable_from(
            &repo,
            &request.commit_hash,
            &context.range.head_commit,
        )
        .map_err(map_analysis_error)?;
        if !commit_reachable {
            return Err(command_error_with_detail(
                "COMMIT_NOT_REACHABLE",
                "该提交无法从当前分析目标提交追溯",
                request.commit_hash,
            ));
        }

        let commit = revier_analysis::git::commits::get_commit(&repo, &request.commit_hash)
            .map_err(map_analysis_error)?;
        let parent_hash = commit
            .parents
            .first()
            .cloned()
            .unwrap_or_else(|| repo.empty_tree().id.to_string());
        let path_candidates = revier_analysis::git::diff::connected_paths_between(
            &repo,
            &request.commit_hash,
            &context.range.head_commit,
            &file.path,
            file.old_path.as_deref(),
        )
        .map_err(map_analysis_error)?;
        let change = revier_analysis::git::diff::commit_file_changes(&repo, &request.commit_hash)
            .map_err(map_analysis_error)?
            .into_iter()
            .find(|change| {
                change.parent_index == 0 && change_matches_paths(change, &path_candidates)
            })
            .ok_or_else(|| {
                command_error_with_detail(
                    "FILE_NOT_CHANGED_IN_COMMIT",
                    "该提交未修改当前文件",
                    request.file_path.clone(),
                )
            })?;
        let old_path = old_text_path(&change);
        let new_path = new_text_path(&change);
        let old_bytes = match old_path {
            Some(path) => require_blob_at_commit(
                revier_analysis::git::blob::read_blob_at_commit(&repo, &parent_hash, path),
                &parent_hash,
                path,
            )?,
            None => Vec::new(),
        };
        let new_bytes = match new_path {
            Some(path) => require_blob_at_commit(
                revier_analysis::git::blob::read_blob_at_commit(&repo, &request.commit_hash, path),
                &request.commit_hash,
                path,
            )?,
            None => Vec::new(),
        };
        let requested_encoding = request.encoding.unwrap_or(TextEncoding::Auto);
        let preferred_bytes = if new_path.is_some() {
            &new_bytes
        } else {
            &old_bytes
        };
        let resolved_encoding =
            revier_analysis::text_encoding::decode_text_bytes(preferred_bytes, requested_encoding)
                .map_err(map_analysis_error)?
                .encoding;
        let concrete_encoding =
            revier_analysis::text_encoding::resolved_as_requested(resolved_encoding);
        let old_text =
            revier_analysis::text_encoding::decode_text_bytes(&old_bytes, concrete_encoding)
                .map_err(map_analysis_error)?
                .text;
        let new_text =
            revier_analysis::text_encoding::decode_text_bytes(&new_bytes, concrete_encoding)
                .map_err(map_analysis_error)?
                .text;
        if change.is_binary
            && !matches!(
                resolved_encoding,
                ResolvedTextEncoding::Utf16Le | ResolvedTextEncoding::Utf16Be
            )
        {
            return Err(command_error_with_detail(
                "FILE_NOT_ANALYZABLE",
                "文件包含二进制内容",
                change.path,
            ));
        }
        let diff = revier_analysis::overlay::diff_builder::build_overlay_diff(&old_text, &new_text);
        let cached_blocks = diff
            .blocks
            .iter()
            .enumerate()
            .map(|(ordinal, block)| cached_commit_overlay_block(ordinal as u32, block))
            .collect::<CommandResult<Vec<_>>>()?;
        let rows = adapt_rows(diff.rows)?;
        let related_commit = RelatedCommit {
            hash: commit.hash,
            short_hash: commit.short_hash,
            author_name: commit.author_name,
            author_email: commit.author_email,
            committed_at: commit.committed_at,
            subject: commit.subject,
            matched_by_filter: false,
            touched_ranges: Vec::new(),
            attribution: None,
        };
        let author = AuthorSummary {
            name: related_commit.author_name.clone(),
            email: related_commit.author_email.clone(),
            commit_count: 1,
            last_committed_at: related_commit.committed_at.clone(),
        };
        let mut blocks = adapt_blocks(diff.blocks)?;
        for block in &mut blocks {
            block.authors = vec![author.clone()];
            block.related_commits = vec![related_commit.clone()];
        }

        let overlay = FileOverlay {
            mode: Some(FileOverlayMode::Commit),
            file,
            range: context.range.clone(),
            rows: Some(rows),
            blocks,
            warnings: Vec::new(),
            commit: Some(related_commit),
            parent_hash: Some(parent_hash.clone()),
            old_content: old_text,
            new_content: new_text,
            resolved_encoding,
        };
        if let Some(file_analysis) = file_analysis {
            let cached = CachedCommitOverlay {
                commit_overlay_id: uuid::Uuid::new_v4().to_string(),
                file_analysis_id: file_analysis.file_analysis_id,
                commit_hash: request.commit_hash,
                parent_hash,
                historical_path: change.path,
                old_blob_id: change.old_blob_id,
                new_blob_id: change.new_blob_id,
                resolved_encoding: resolved_encoding_code(resolved_encoding),
                analysis_version: revier_analysis::cache::ANALYSIS_VERSION,
                elapsed_ms: started.elapsed().as_millis() as u64,
                completed_at: Utc::now().to_rfc3339(),
                blocks: cached_blocks,
            };
            let (_, conn) = cache_connection_for_context(&context, &self.database_registry)?;
            revier_analysis::cache::repository::replace_commit_overlay(&conn, &cached)
                .map_err(map_analysis_error)?;
        }
        Ok(CommitOverlayLoadResult {
            overlay,
            cache_state: if matches!(request.cache_mode, CacheMode::Refresh) {
                CacheState::Refresh
            } else {
                CacheState::Miss
            },
        })
    }

    fn run_analysis(
        &self,
        projects: &ProjectService,
        task_id: &str,
        filters: ReviewFilters,
    ) -> CommandResult<AnalysisRunResult> {
        let analysis_started_at = Utc::now().to_rfc3339();
        let analysis_started = Instant::now();
        let context = self.analysis_context_for_task(task_id);
        self.ensure_not_cancelled(task_id)?;
        self.mark_running(task_id, AnalysisStage::ReadRepository, "读取项目仓库");
        let project = projects.get_project(&filters.project_id)?;
        let repo_path = PathBuf::from(&project.repo_path);
        let validation =
            revier_analysis::api::validate_repository(&repo_path).map_err(map_analysis_error)?;
        if !validation.valid {
            return Err(command_error_with_detail(
                "REPOSITORY_INVALID",
                validation
                    .error
                    .unwrap_or_else(|| "项目仓库无效".to_string()),
                project.repo_path,
            ));
        }

        self.ensure_not_cancelled(task_id)?;
        self.mark_running(task_id, AnalysisStage::ResolveRange, "解析分析范围");
        let range = revier_analysis::api::resolve_analysis_range_with_context(
            &repo_path,
            &filters.branch,
            filters.start_at.clone(),
            filters.end_at.clone(),
            &context,
        )
        .map_err(map_analysis_error)?;

        self.ensure_not_cancelled(task_id)?;
        self.mark_running(task_id, AnalysisStage::LoadChangedFiles, "准备仓库索引");
        ensure_analysis_index(&repo_path, &range.branch, &context, &self.database_registry)
            .map_err(map_analysis_error)?;

        self.ensure_not_cancelled(task_id)?;
        self.mark_running(task_id, AnalysisStage::LoadChangedFiles, "读取变更文件");
        let repo = revier_analysis::git::repository::open_repository(&repo_path)
            .map_err(map_analysis_error)?;
        let identity = revier_analysis::git::repository::repository_identity(&repo)
            .map_err(map_analysis_error)?;
        let db_path = revier_analysis::index::connection::default_database_path(&identity.repo_id)
            .map_err(map_analysis_error)?;
        let conn = self
            .database_registry
            .connect(&db_path)
            .map_err(map_analysis_error)?;
        let output = revier_analysis::api::query_files_with_connection(
            QueryFilesRequest {
                repo: repo_path.clone(),
                db: None,
                base: range.base_commit.clone(),
                head: range.head_commit.clone(),
                branch: range.branch.clone(),
                authors: filters.author_keys.clone().unwrap_or_default(),
                author_query: filters.author_query.clone(),
                message: filters.message_query.clone(),
                since: range.start_at.clone(),
                until: range.end_at.clone(),
                globs: filters.glob_rules.clone(),
            },
            &conn,
            &context,
        )
        .map_err(map_analysis_error)?;

        self.ensure_not_cancelled(task_id)?;
        let locally_reachable =
            revier_analysis::git::commits::all_local_branch_reachable_hashes(&repo, &context)
                .map_err(map_analysis_error)?;
        let cache_files = output
            .files
            .iter()
            .map(cached_analysis_file)
            .collect::<CommandResult<Vec<_>>>()?;
        let files = output
            .files
            .iter()
            .cloned()
            .map(adapt_changed_file)
            .collect::<CommandResult<Vec<_>>>()?;
        let author_keys = filters.author_keys.clone().unwrap_or_default();
        let author_part = author_keys.join("\0");
        let glob_part = filters.glob_rules.join("\0");
        let filter_fingerprint = revier_analysis::cache::filter_fingerprint(&[
            &range.branch,
            range.start_at.as_deref().unwrap_or_default(),
            range.end_at.as_deref().unwrap_or_default(),
            &author_part,
            filters.author_query.as_deref().unwrap_or_default(),
            filters.message_query.as_deref().unwrap_or_default(),
            &glob_part,
        ]);
        let snapshot = CachedAnalysisSnapshot {
            analysis_id: uuid::Uuid::new_v4().to_string(),
            repo_id: identity.repo_id.clone(),
            branch: range.branch.clone(),
            base_commit: range.base_commit.clone(),
            head_commit: range.head_commit.clone(),
            start_at: range.start_at.clone(),
            end_at: range.end_at.clone(),
            author_query: filters.author_query.clone(),
            message_query: filters.message_query.clone(),
            filter_fingerprint,
            analysis_version: revier_analysis::cache::ANALYSIS_VERSION,
            started_at: analysis_started_at,
            completed_at: Utc::now().to_rfc3339(),
            elapsed_ms: analysis_started.elapsed().as_millis() as u64,
            last_selected_path: None,
            author_keys,
            globs: filters.glob_rules.clone(),
            files: cache_files,
        };
        self.ensure_not_cancelled(task_id)?;
        context.check_cancelled().map_err(map_analysis_error)?;
        revier_analysis::index::writer::checkpoint(&conn).map_err(map_analysis_error)?;
        context.report_progress(OperationProgressUpdate {
            stage: OperationStage::PublishCache,
            message: format!("发布项目缓存：{}", range.branch),
            completed_units: Some(0),
            total_units: Some(1),
        });
        revier_analysis::cache::repository::publish_branch_snapshot_and_prune_with_completion(
            &conn,
            &snapshot,
            &locally_reachable,
            || {
                (
                    Utc::now().to_rfc3339(),
                    analysis_started.elapsed().as_millis() as u64,
                )
            },
        )
        .map_err(map_analysis_error)?;
        context.report_progress(OperationProgressUpdate {
            stage: OperationStage::PublishCache,
            message: format!("项目缓存发布完成：{}", range.branch),
            completed_units: Some(1),
            total_units: Some(1),
        });

        Ok(AnalysisRunResult {
            files,
            context: ReviewTaskContext {
                project,
                filters,
                range,
            },
        })
    }

    fn mark_failed(&self, task_id: &str, error: revier_analysis::contracts::AppError) {
        if let Some(task) = self.tasks.lock().expect("任务锁被污染").get_mut(task_id) {
            if matches!(task.status, AnalysisTaskStatus::Cancelled) {
                self.clear_task_results(task_id);
                self.cleanup_cancellation(task_id);
                return;
            }
            task.status = AnalysisTaskStatus::Failed;
            task.error = Some(error.clone());
            task.message = Some(error.message);
        }
    }

    fn ensure_not_cancelled(&self, task_id: &str) -> CommandResult<()> {
        if self.is_cancelled(task_id) {
            Err(command_error(
                "TASK_CANCELLED",
                format!("任务已取消：{task_id}"),
            ))
        } else {
            Ok(())
        }
    }

    fn analysis_context_for_task(&self, task_id: &str) -> AnalysisExecutionContext {
        let registration = self
            .progress_by_task
            .lock()
            .expect("进度注册锁被污染")
            .get(task_id)
            .cloned();
        let reporter = Arc::new(TaskProgressReporter {
            tasks: Arc::clone(&self.tasks),
            task_id: task_id.to_string(),
            started: Instant::now(),
            registration,
            emission: Mutex::new(ProgressEmissionState::default()),
        });
        self.cancellations_by_task
            .lock()
            .expect("取消令牌锁被污染")
            .get(task_id)
            .cloned()
            .map(|token| {
                AnalysisExecutionContext::with_cancel_and_progress(
                    move || token.is_cancelled(),
                    reporter.clone(),
                )
            })
            .unwrap_or_else(|| AnalysisExecutionContext::with_progress(reporter))
    }

    fn is_cancelled(&self, task_id: &str) -> bool {
        let status_cancelled = self
            .tasks
            .lock()
            .expect("任务锁被污染")
            .get(task_id)
            .is_some_and(|task| matches!(task.status, AnalysisTaskStatus::Cancelled));
        if status_cancelled {
            return true;
        }

        self.cancellations_by_task
            .lock()
            .expect("取消令牌锁被污染")
            .get(task_id)
            .is_some_and(CancellationToken::is_cancelled)
    }

    fn cleanup_cancellation(&self, task_id: &str) {
        self.cancellations_by_task
            .lock()
            .expect("取消令牌锁被污染")
            .remove(task_id);
    }

    fn clear_task_results(&self, task_id: &str) {
        self.files_by_task
            .lock()
            .expect("文件缓存锁被污染")
            .remove(task_id);
        self.contexts_by_task
            .lock()
            .expect("任务上下文锁被污染")
            .remove(task_id);
    }

    fn ensure_completed_task(&self, task_id: &str) -> CommandResult<()> {
        let task = self.get_task(task_id)?;
        match &task.status {
            AnalysisTaskStatus::Completed => Ok(()),
            AnalysisTaskStatus::Failed => Err(command_error_with_detail(
                "TASK_FAILED",
                format!("任务执行失败：{task_id}"),
                task.error
                    .as_ref()
                    .map(error_detail)
                    .unwrap_or_else(|| "任务失败原因未知".to_string()),
            )),
            status => Err(command_error_with_detail(
                "TASK_NOT_COMPLETED",
                format!("任务尚未完成：{task_id}"),
                status_label(status),
            )),
        }
    }

    fn context_for_task(&self, task_id: &str) -> CommandResult<ReviewTaskContext> {
        self.contexts_by_task
            .lock()
            .expect("任务上下文锁被污染")
            .get(task_id)
            .cloned()
            .ok_or_else(|| {
                command_error(
                    "TASK_CONTEXT_NOT_FOUND",
                    format!("任务上下文缓存不存在：{task_id}"),
                )
            })
    }

    fn file_for_task(&self, task_id: &str, file_path: &str) -> CommandResult<ChangedFile> {
        self.files_by_task
            .lock()
            .expect("文件缓存锁被污染")
            .get(task_id)
            .and_then(|files| {
                files
                    .iter()
                    .find(|file| {
                        file.path == file_path || file.old_path.as_deref() == Some(file_path)
                    })
                    .cloned()
            })
            .ok_or_else(|| {
                command_error_with_detail(
                    "TASK_FILE_NOT_FOUND",
                    "任务文件缓存中不存在请求的文件",
                    file_path,
                )
            })
    }
}

fn ensure_analysis_index(
    repo_path: &Path,
    branch: &str,
    context: &AnalysisExecutionContext,
    database_registry: &DatabaseRegistry,
) -> Result<(), AnalysisAppError> {
    let repo = revier_analysis::git::repository::open_repository(repo_path)?;
    let identity = revier_analysis::git::repository::repository_identity(&repo)?;
    let db_path = revier_analysis::index::connection::default_database_path(&identity.repo_id)?;
    let connection = database_registry.connect(&db_path)?;
    revier_analysis::commands::index_build::run_with_connection(
        IndexBuildArgs {
            common: IndexCommonArgs {
                repo: repo_path.to_path_buf(),
                db: Some(db_path),
                format: OutputFormat::Json,
                pretty: false,
            },
            branch: branch.to_string(),
        },
        &connection,
        context,
    )?;
    Ok(())
}

fn branch_cache_location(
    project: &ReviewProject,
    branch: &str,
) -> CommandResult<(String, String, PathBuf)> {
    let repo = revier_analysis::git::repository::open_repository(Path::new(&project.repo_path))
        .map_err(map_analysis_error)?;
    let identity =
        revier_analysis::git::repository::repository_identity(&repo).map_err(map_analysis_error)?;
    let current_head = repo
        .rev_parse_single(branch)
        .map_err(|error| command_error("BRANCH_NOT_FOUND", error.to_string()))?
        .detach()
        .to_string();
    let db_path = revier_analysis::index::connection::default_database_path(&identity.repo_id)
        .map_err(map_analysis_error)?;
    Ok((identity.repo_id, current_head, db_path))
}

fn ensure_file_refresh_is_current(
    context: &ReviewTaskContext,
    cache_mode: CacheMode,
) -> CommandResult<()> {
    if !matches!(cache_mode, CacheMode::Refresh) {
        return Ok(());
    }
    let (_, current_head, _) = branch_cache_location(&context.project, &context.range.branch)?;
    if current_head != context.range.head_commit {
        return Err(command_error_with_detail(
            "BRANCH_CACHE_STALE",
            "项目缓存已过期，请先重新分析项目",
            format!(
                "cached_head={}; current_head={current_head}",
                context.range.head_commit
            ),
        ));
    }
    Ok(())
}

fn empty_branch_restore(
    project_id: &str,
    branch: &str,
    current_head: String,
    cache_read_elapsed_ms: u64,
) -> BranchAnalysisRestoreResult {
    BranchAnalysisRestoreResult {
        project_id: project_id.to_string(),
        branch: branch.to_string(),
        cache_state: CacheState::Miss,
        cache_hit: false,
        stale: false,
        task: None,
        range: None,
        filters: None,
        files: Vec::new(),
        last_selected_path: None,
        current_head,
        cached_head: None,
        cache_read_elapsed_ms,
        analysis_elapsed_ms: None,
    }
}

fn overlay_common_args(context: &ReviewTaskContext) -> OverlayCommonArgs {
    OverlayCommonArgs {
        repo: PathBuf::from(&context.project.repo_path),
        db: None,
        base: context.range.base_commit.clone(),
        head: context.range.head_commit.clone(),
        branch: context.range.branch.clone(),
        globs: context.filters.glob_rules.clone(),
        authors: context.filters.author_keys.clone().unwrap_or_default(),
        author_query: context.filters.author_query.clone(),
        message: context.filters.message_query.clone(),
        require_index: false,
        format: OutputFormat::Json,
        pretty: false,
    }
}

struct OverlayDocument {
    analysis_id: String,
    change: revier_analysis::git::diff::CommitFileChange,
    old_text: String,
    new_text: String,
    resolved_encoding: ResolvedTextEncoding,
    warnings: Vec<revier_analysis::contracts::AppError>,
}

fn cache_connection_for_context(
    context: &ReviewTaskContext,
    database_registry: &DatabaseRegistry,
) -> CommandResult<(String, duckdb::Connection)> {
    let repo =
        revier_analysis::git::repository::open_repository(Path::new(&context.project.repo_path))
            .map_err(map_analysis_error)?;
    let identity =
        revier_analysis::git::repository::repository_identity(&repo).map_err(map_analysis_error)?;
    let db_path = revier_analysis::index::connection::default_database_path(&identity.repo_id)
        .map_err(map_analysis_error)?;
    let conn = database_registry
        .connect(&db_path)
        .map_err(map_analysis_error)?;
    revier_analysis::index::migrations::ensure_compatible_schema(&conn)
        .map_err(map_analysis_error)?;
    Ok((identity.repo_id, conn))
}

fn load_range_overlay_document(
    context: &ReviewTaskContext,
    file_path: &str,
    requested_encoding: TextEncoding,
    database_registry: &DatabaseRegistry,
) -> CommandResult<OverlayDocument> {
    let repo =
        revier_analysis::git::repository::open_repository(Path::new(&context.project.repo_path))
            .map_err(map_analysis_error)?;
    let identity =
        revier_analysis::git::repository::repository_identity(&repo).map_err(map_analysis_error)?;
    let db_path = revier_analysis::index::connection::default_database_path(&identity.repo_id)
        .map_err(map_analysis_error)?;
    let conn = database_registry
        .connect(&db_path)
        .map_err(map_analysis_error)?;
    revier_analysis::index::migrations::ensure_compatible_schema(&conn)
        .map_err(map_analysis_error)?;
    let (analysis_id, cached_file) = revier_analysis::cache::repository::load_branch_analysis_file(
        &conn,
        &identity.repo_id,
        &context.range.branch,
        file_path,
    )
    .map_err(map_analysis_error)?
    .ok_or_else(|| {
        command_error_with_detail(
            "FILE_CACHE_NOT_FOUND",
            "分支快照中不存在目标文件缓存",
            file_path,
        )
    })?;
    let status = cached_file_status_label(cached_file.status)?;
    let change = revier_analysis::git::diff::CommitFileChange {
        commit_hash: context.range.head_commit.clone(),
        parent_hash: context.range.base_commit.clone(),
        parent_index: 0,
        path: cached_file.path.clone(),
        old_path: cached_file.old_path.clone(),
        status: status.to_string(),
        additions: cached_file.additions,
        deletions: cached_file.deletions,
        is_binary: cached_file.is_binary,
        is_previewable: cached_file.is_previewable,
        similarity: None,
        old_blob_id: cached_file.old_blob_id.clone(),
        new_blob_id: cached_file.new_blob_id.clone(),
    };
    let old_bytes = cached_file
        .old_blob_id
        .as_deref()
        .map(|blob_id| revier_analysis::git::blob::read_blob_by_id(&repo, blob_id))
        .transpose()
        .map_err(map_analysis_error)?
        .unwrap_or_default();
    let new_bytes = cached_file
        .new_blob_id
        .as_deref()
        .map(|blob_id| revier_analysis::git::blob::read_blob_by_id(&repo, blob_id))
        .transpose()
        .map_err(map_analysis_error)?
        .unwrap_or_default();
    let new_path = new_text_path(&change);
    let preferred_bytes = if new_path.is_some() {
        &new_bytes
    } else {
        &old_bytes
    };
    let resolved_encoding =
        revier_analysis::text_encoding::decode_text_bytes(preferred_bytes, requested_encoding)
            .map_err(map_analysis_error)?
            .encoding;
    let concrete_encoding =
        revier_analysis::text_encoding::resolved_as_requested(resolved_encoding);
    let old_text = revier_analysis::text_encoding::decode_text_bytes(&old_bytes, concrete_encoding)
        .map_err(map_analysis_error)?
        .text;
    let new_text = revier_analysis::text_encoding::decode_text_bytes(&new_bytes, concrete_encoding)
        .map_err(map_analysis_error)?
        .text;
    if change.is_binary
        && !matches!(
            resolved_encoding,
            ResolvedTextEncoding::Utf16Le | ResolvedTextEncoding::Utf16Be
        )
    {
        return Err(command_error_with_detail(
            "FILE_NOT_ANALYZABLE",
            "文件包含二进制内容",
            change.path,
        ));
    }

    Ok(OverlayDocument {
        analysis_id,
        change,
        old_text,
        new_text,
        resolved_encoding,
        warnings: Vec::new(),
    })
}

fn resolved_encoding_as_requested(encoding: ResolvedTextEncoding) -> TextEncoding {
    match encoding {
        ResolvedTextEncoding::Utf8 => TextEncoding::Utf8,
        ResolvedTextEncoding::Gb18030 => TextEncoding::Gb18030,
        ResolvedTextEncoding::Utf16Le => TextEncoding::Utf16Le,
        ResolvedTextEncoding::Utf16Be => TextEncoding::Utf16Be,
    }
}

fn append_app_warning(warnings: &mut Vec<revier_analysis::contracts::AppError>, message: &str) {
    let warning = command_error("ANALYSIS_WARNING", message.to_string());
    if !warnings
        .iter()
        .any(|item| item.code == warning.code && item.message == warning.message)
    {
        warnings.push(warning);
    }
}

fn parse_optional_time(value: Option<&str>, field: &str) -> CommandResult<Option<DateTime<Utc>>> {
    value.map(|value| parse_time(value, field)).transpose()
}

fn parse_time(value: &str, field: &str) -> CommandResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| {
            command_error("INVALID_ARGUMENT", format!("{field} 时间格式无效：{error}"))
        })
}

fn parse_commit_time(
    commit: &revier_analysis::git::commits::IndexedCommit,
) -> CommandResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&commit.committed_at)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| {
            command_error_with_detail(
                "REPOSITORY_ERROR",
                "提交时间格式无效",
                format!("{}: {error}", commit.hash),
            )
        })
}

fn change_matches_paths(
    change: &revier_analysis::git::diff::CommitFileChange,
    paths: &[String],
) -> bool {
    paths
        .iter()
        .any(|path| change.path == *path || change.old_path.as_deref() == Some(path.as_str()))
}

fn old_text_path(change: &revier_analysis::git::diff::CommitFileChange) -> Option<&str> {
    match change.status.as_str() {
        "added" => None,
        "deleted" | "modified" => Some(change.old_path.as_deref().unwrap_or(&change.path)),
        "renamed" => Some(change.old_path.as_deref().unwrap_or(&change.path)),
        _ => Some(change.old_path.as_deref().unwrap_or(&change.path)),
    }
}

fn new_text_path(change: &revier_analysis::git::diff::CommitFileChange) -> Option<&str> {
    match change.status.as_str() {
        "deleted" => None,
        _ => Some(&change.path),
    }
}

fn require_blob_at_commit(
    blob: Result<Option<Vec<u8>>, AnalysisAppError>,
    commit_hash: &str,
    path: &str,
) -> CommandResult<Vec<u8>> {
    blob.map_err(map_analysis_error)?.ok_or_else(|| {
        command_error_with_detail(
            "BLOB_NOT_FOUND",
            "提交中的文件 Blob 不存在",
            format!("commit={commit_hash}; path={path}"),
        )
    })
}

fn adapt_changed_file(file: ChangedFileOutput) -> CommandResult<ChangedFile> {
    Ok(ChangedFile {
        path: file.path,
        old_path: file.old_path,
        status: adapt_changed_file_status(&file.status)?,
        additions: file.additions,
        deletions: file.deletions,
        is_binary: file.is_binary,
        is_previewable: file.is_previewable,
    })
}

fn cached_analysis_file(file: &ChangedFileOutput) -> CommandResult<CachedAnalysisFile> {
    Ok(CachedAnalysisFile {
        path: file.path.clone(),
        old_path: file.old_path.clone(),
        status: changed_file_status_code(&file.status)?,
        additions: file.additions,
        deletions: file.deletions,
        is_binary: file.is_binary,
        is_previewable: file.is_previewable,
        old_blob_id: file.old_blob_id.clone(),
        new_blob_id: file.new_blob_id.clone(),
    })
}

fn adapt_cached_analysis_file(file: &CachedAnalysisFile) -> CommandResult<ChangedFile> {
    let status = match file.status {
        0 => ChangedFileStatus::Added,
        1 => ChangedFileStatus::Modified,
        2 => ChangedFileStatus::Deleted,
        3 => ChangedFileStatus::Renamed,
        4 => ChangedFileStatus::Binary,
        other => {
            return Err(command_error_with_detail(
                "UNKNOWN_CHANGED_FILE_STATUS",
                format!("未知缓存文件状态：{other}"),
                other.to_string(),
            ))
        }
    };
    Ok(ChangedFile {
        path: file.path.clone(),
        old_path: file.old_path.clone(),
        status,
        additions: file.additions,
        deletions: file.deletions,
        is_binary: file.is_binary,
        is_previewable: file.is_previewable,
    })
}

fn changed_file_status_code(status: &str) -> CommandResult<i16> {
    match status {
        "added" => Ok(0),
        "modified" => Ok(1),
        "deleted" => Ok(2),
        "renamed" => Ok(3),
        "binary" => Ok(4),
        other => Err(command_error_with_detail(
            "UNKNOWN_CHANGED_FILE_STATUS",
            format!("未知变更文件状态：{other}"),
            other,
        )),
    }
}

fn cached_file_status_label(status: i16) -> CommandResult<&'static str> {
    match status {
        0 => Ok("added"),
        1 => Ok("modified"),
        2 => Ok("deleted"),
        3 => Ok("renamed"),
        4 => Ok("binary"),
        other => Err(command_error_with_detail(
            "UNKNOWN_CHANGED_FILE_STATUS",
            format!("未知缓存文件状态：{other}"),
            other.to_string(),
        )),
    }
}

fn adapt_changed_file_status(status: &str) -> CommandResult<ChangedFileStatus> {
    match status {
        "added" => Ok(ChangedFileStatus::Added),
        "modified" => Ok(ChangedFileStatus::Modified),
        "deleted" => Ok(ChangedFileStatus::Deleted),
        "renamed" => Ok(ChangedFileStatus::Renamed),
        "binary" => Ok(ChangedFileStatus::Binary),
        other => Err(command_error_with_detail(
            "UNKNOWN_CHANGED_FILE_STATUS",
            format!("未知变更文件状态：{other}"),
            other,
        )),
    }
}

#[cfg(test)]
fn adapt_file_overlay_output(
    output: revier_analysis::json::FileOverlayCommandOutput,
    cached_range: &AnalysisRange,
) -> CommandResult<FileOverlay> {
    let overlay = output.overlay;
    let resolved_encoding = adapt_resolved_text_encoding(&overlay.resolved_encoding)?;
    Ok(FileOverlay {
        mode: Some(adapt_file_overlay_mode(&overlay.mode)?),
        file: adapt_changed_file(overlay.file)?,
        range: cached_range.clone(),
        rows: Some(adapt_rows(overlay.rows)?),
        blocks: adapt_blocks(overlay.blocks)?,
        warnings: overlay
            .warnings
            .into_iter()
            .map(|warning| command_error("ANALYSIS_WARNING", warning))
            .collect(),
        commit: None,
        parent_hash: None,
        old_content: overlay.old_content,
        new_content: overlay.new_content,
        resolved_encoding,
    })
}

#[cfg(test)]
fn adapt_resolved_text_encoding(encoding: &str) -> CommandResult<ResolvedTextEncoding> {
    match encoding {
        "utf-8" => Ok(ResolvedTextEncoding::Utf8),
        "gb18030" => Ok(ResolvedTextEncoding::Gb18030),
        "utf-16le" => Ok(ResolvedTextEncoding::Utf16Le),
        "utf-16be" => Ok(ResolvedTextEncoding::Utf16Be),
        other => Err(command_error_with_detail(
            "UNKNOWN_RESOLVED_TEXT_ENCODING",
            format!("未知已解析文本编码：{other}"),
            other,
        )),
    }
}

#[cfg(test)]
fn adapt_file_overlay_mode(mode: &str) -> CommandResult<FileOverlayMode> {
    match mode {
        "range" => Ok(FileOverlayMode::Range),
        "commit" => Ok(FileOverlayMode::Commit),
        other => Err(command_error_with_detail(
            "UNKNOWN_FILE_OVERLAY_MODE",
            format!("未知 overlay 模式：{other}"),
            other,
        )),
    }
}

fn adapt_rows(rows: Vec<SideBySideDiffRowOutput>) -> CommandResult<Vec<SideBySideDiffRow>> {
    rows.into_iter().map(adapt_row).collect()
}

fn adapt_row(row: SideBySideDiffRowOutput) -> CommandResult<SideBySideDiffRow> {
    Ok(SideBySideDiffRow {
        old_line_number: row.old_line_number.map(|value| value as u64),
        new_line_number: row.new_line_number.map(|value| value as u64),
        old_text: row.old_text,
        new_text: row.new_text,
        r#type: adapt_row_type(&row.r#type)?,
        word_changes: row.word_changes.map(adapt_word_changes).transpose()?,
        block_id: row.block_id,
    })
}

fn adapt_row_type(row_type: &str) -> CommandResult<SideBySideDiffRowType> {
    match row_type {
        "context" => Ok(SideBySideDiffRowType::Context),
        "added" => Ok(SideBySideDiffRowType::Added),
        "deleted" => Ok(SideBySideDiffRowType::Deleted),
        "modified" => Ok(SideBySideDiffRowType::Modified),
        other => Err(command_error_with_detail(
            "UNKNOWN_DIFF_ROW_TYPE",
            format!("未知 diff 行类型：{other}"),
            other,
        )),
    }
}

fn adapt_word_changes(changes: Vec<WordChangeOutput>) -> CommandResult<Vec<WordChange>> {
    Ok(changes
        .into_iter()
        .map(|change| WordChange {
            value: change.value,
            added: change.added,
            removed: change.removed,
        })
        .collect())
}

fn adapt_blocks(blocks: Vec<DiffBlockOutput>) -> CommandResult<Vec<DiffBlock>> {
    blocks.into_iter().map(adapt_block).collect()
}

fn adapt_block_attributions(
    blocks: Vec<DiffBlockOutput>,
) -> CommandResult<Vec<DiffBlockAttribution>> {
    blocks
        .into_iter()
        .map(|block| {
            let authors = adapt_authors(block.authors, &block.related_commits);
            Ok(DiffBlockAttribution {
                id: block.id,
                authors,
                related_commits: adapt_related_commits(block.related_commits)?,
                attribution: block.attribution.map(adapt_block_attribution).transpose()?,
            })
        })
        .collect()
}

fn cached_file_analysis(
    analysis_id: &str,
    path: &str,
    encoding: ResolvedTextEncoding,
    block_signature: &str,
    content_elapsed_ms: u64,
    attribution_elapsed_ms: u64,
    blocks: &[DiffBlockOutput],
) -> CommandResult<CachedFileAnalysis> {
    Ok(CachedFileAnalysis {
        file_analysis_id: uuid::Uuid::new_v4().to_string(),
        analysis_id: analysis_id.to_string(),
        path: path.to_string(),
        resolved_encoding: resolved_encoding_code(encoding),
        block_signature: block_signature.to_string(),
        analysis_version: revier_analysis::cache::ANALYSIS_VERSION,
        content_elapsed_ms,
        attribution_elapsed_ms,
        completed_at: Utc::now().to_rfc3339(),
        blocks: blocks
            .iter()
            .enumerate()
            .map(|(ordinal, block)| cached_file_block(ordinal as u32, block))
            .collect::<CommandResult<Vec<_>>>()?,
    })
}

fn cached_file_block(ordinal: u32, block: &DiffBlockOutput) -> CommandResult<CachedFileBlock> {
    Ok(CachedFileBlock {
        ordinal,
        old_start: block.old_start as u64,
        old_end: block.old_end as u64,
        new_start: block.new_start as u64,
        new_end: block.new_end as u64,
        change_type: diff_block_change_type_code(&block.change_type)?,
        confidence: block
            .attribution
            .as_ref()
            .map(|value| attribution_confidence_code(&value.confidence))
            .transpose()?,
        warning_flags: block
            .attribution
            .as_ref()
            .map(|value| attribution_warning_flags(&value.warnings))
            .unwrap_or_default(),
        commits: block
            .related_commits
            .iter()
            .map(|commit| {
                Ok(CachedBlockCommit {
                    commit_hash: commit.hash.clone(),
                    matched_by_filter: commit.matched_by_filter,
                    attribution_method: commit
                        .attribution
                        .as_ref()
                        .map(|value| attribution_method_code(&value.method))
                        .transpose()?,
                    touched_ranges: commit
                        .touched_ranges
                        .iter()
                        .map(|range| CachedTouchedRange {
                            old_start: range.old_start.map(|value| value as u64),
                            old_end: range.old_end.map(|value| value as u64),
                            new_start: range.new_start.map(|value| value as u64),
                            new_end: range.new_end.map(|value| value as u64),
                        })
                        .collect(),
                    merge_hashes: commit
                        .attribution
                        .as_ref()
                        .map(|value| value.via_merge_hashes.clone())
                        .unwrap_or_default(),
                })
            })
            .collect::<CommandResult<Vec<_>>>()?,
    })
}

fn cached_commit_overlay_block(
    ordinal: u32,
    block: &DiffBlockOutput,
) -> CommandResult<CachedCommitOverlayBlock> {
    Ok(CachedCommitOverlayBlock {
        ordinal,
        old_start: block.old_start as u64,
        old_end: block.old_end as u64,
        new_start: block.new_start as u64,
        new_end: block.new_end as u64,
        change_type: diff_block_change_type_code(&block.change_type)?,
    })
}

fn restore_cached_commit_overlay(
    context: &ReviewTaskContext,
    file: &ChangedFile,
    cached: &CachedCommitOverlay,
    requested_encoding: TextEncoding,
) -> CommandResult<Option<FileOverlay>> {
    if cached.historical_path.is_empty()
        || (cached.old_blob_id.is_none() && cached.new_blob_id.is_none())
    {
        return Ok(None);
    }
    let repo =
        revier_analysis::git::repository::open_repository(Path::new(&context.project.repo_path))
            .map_err(map_analysis_error)?;
    let commit = revier_analysis::git::commits::get_commit(&repo, &cached.commit_hash)
        .map_err(map_analysis_error)?;
    let actual_parent = commit
        .parents
        .first()
        .cloned()
        .unwrap_or_else(|| repo.empty_tree().id.to_string());
    if actual_parent != cached.parent_hash {
        return Ok(None);
    }
    let old_bytes = cached
        .old_blob_id
        .as_deref()
        .map(|blob_id| revier_analysis::git::blob::read_blob_by_id(&repo, blob_id))
        .transpose()
        .map_err(map_analysis_error)?
        .unwrap_or_default();
    let new_bytes = cached
        .new_blob_id
        .as_deref()
        .map(|blob_id| revier_analysis::git::blob::read_blob_by_id(&repo, blob_id))
        .transpose()
        .map_err(map_analysis_error)?
        .unwrap_or_default();
    let preferred_bytes = if cached.new_blob_id.is_some() {
        &new_bytes
    } else {
        &old_bytes
    };
    let resolved_encoding =
        revier_analysis::text_encoding::decode_text_bytes(preferred_bytes, requested_encoding)
            .map_err(map_analysis_error)?
            .encoding;
    if resolved_encoding_code(resolved_encoding) != cached.resolved_encoding {
        return Ok(None);
    }
    let concrete_encoding =
        revier_analysis::text_encoding::resolved_as_requested(resolved_encoding);
    let old_text = revier_analysis::text_encoding::decode_text_bytes(&old_bytes, concrete_encoding)
        .map_err(map_analysis_error)?
        .text;
    let new_text = revier_analysis::text_encoding::decode_text_bytes(&new_bytes, concrete_encoding)
        .map_err(map_analysis_error)?
        .text;
    if cached.blocks.iter().enumerate().any(|(ordinal, block)| {
        block.ordinal != ordinal as u32
            || block.old_start > block.old_end
            || block.new_start > block.new_end
    }) {
        return Ok(None);
    }
    let related_commit = RelatedCommit {
        hash: commit.hash,
        short_hash: commit.short_hash,
        author_name: commit.author_name,
        author_email: commit.author_email,
        committed_at: commit.committed_at,
        subject: commit.subject,
        matched_by_filter: false,
        touched_ranges: Vec::new(),
        attribution: None,
    };
    let author = AuthorSummary {
        name: related_commit.author_name.clone(),
        email: related_commit.author_email.clone(),
        commit_count: 1,
        last_committed_at: related_commit.committed_at.clone(),
    };
    let blocks = cached
        .blocks
        .iter()
        .map(|block| {
            Ok(DiffBlock {
                id: format!("block-{}", block.ordinal + 1),
                old_start: block.old_start,
                old_end: block.old_end,
                new_start: block.new_start,
                new_end: block.new_end,
                row_start_index: None,
                row_end_index: None,
                change_type: cached_diff_block_change_type(block.change_type)?,
                authors: vec![author.clone()],
                rows: Vec::new(),
                related_commits: vec![related_commit.clone()],
                attribution: None,
            })
        })
        .collect::<CommandResult<Vec<_>>>()?;
    Ok(Some(FileOverlay {
        mode: Some(FileOverlayMode::Commit),
        file: file.clone(),
        range: context.range.clone(),
        rows: None,
        blocks,
        warnings: Vec::new(),
        commit: Some(related_commit),
        parent_hash: Some(cached.parent_hash.clone()),
        old_content: old_text,
        new_content: new_text,
        resolved_encoding,
    }))
}

fn restore_cached_attributions(
    conn: &duckdb::Connection,
    cached: &CachedFileAnalysis,
    requested_blocks: &[revier_analysis::contracts::DiffBlockRange],
) -> CommandResult<Option<Vec<DiffBlockAttribution>>> {
    if cached.blocks.len() != requested_blocks.len()
        || cached
            .blocks
            .iter()
            .zip(requested_blocks)
            .any(|(cached, requested)| {
                cached.old_start != requested.old_start
                    || cached.old_end != requested.old_end
                    || cached.new_start != requested.new_start
                    || cached.new_end != requested.new_end
            })
    {
        return Ok(None);
    }
    let hashes = cached
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .commits
                .iter()
                .map(|commit| commit.commit_hash.clone())
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let metadata = revier_analysis::index::queries::commit_metadata_batch(conn, &hashes)
        .map_err(map_analysis_error)?;
    if metadata.len() != hashes.len() {
        return Ok(None);
    }

    cached
        .blocks
        .iter()
        .zip(requested_blocks)
        .map(|(block, requested)| {
            let related_outputs = block
                .commits
                .iter()
                .map(|commit| cached_related_commit_output(commit, &metadata))
                .collect::<CommandResult<Vec<_>>>()?;
            let authors = related_outputs
                .iter()
                .map(|commit| AuthorOutput {
                    name: commit.author_name.clone(),
                    email: commit.author_email.clone(),
                })
                .collect();
            Ok(DiffBlockAttribution {
                id: requested.id.clone(),
                authors: adapt_authors(authors, &related_outputs),
                related_commits: adapt_related_commits(related_outputs)?,
                attribution: block
                    .confidence
                    .map(|confidence| cached_block_attribution(confidence, block.warning_flags))
                    .transpose()?,
            })
        })
        .collect::<CommandResult<Vec<_>>>()
        .map(Some)
}

fn cached_related_commit_output(
    cached: &CachedBlockCommit,
    metadata: &HashMap<String, revier_analysis::git::commits::IndexedCommit>,
) -> CommandResult<RelatedCommitOutput> {
    let commit = metadata.get(&cached.commit_hash).ok_or_else(|| {
        command_error_with_detail(
            "FILE_CACHE_INCOMPLETE",
            "文件缓存引用的提交元数据不存在",
            &cached.commit_hash,
        )
    })?;
    Ok(RelatedCommitOutput {
        hash: commit.hash.clone(),
        short_hash: commit.short_hash.clone(),
        author_name: commit.author_name.clone(),
        author_email: commit.author_email.clone(),
        committed_at: commit.committed_at.clone(),
        subject: commit.subject.clone(),
        matched_by_filter: cached.matched_by_filter,
        touched_ranges: cached
            .touched_ranges
            .iter()
            .map(|range| TouchedRangeOutput {
                old_start: range.old_start.map(|value| value as usize),
                old_end: range.old_end.map(|value| value as usize),
                new_start: range.new_start.map(|value| value as usize),
                new_end: range.new_end.map(|value| value as usize),
            })
            .collect(),
        attribution: cached
            .attribution_method
            .map(|method| cached_related_attribution(method, &cached.merge_hashes))
            .transpose()?,
    })
}

fn adapt_block(block: DiffBlockOutput) -> CommandResult<DiffBlock> {
    let authors = adapt_authors(block.authors, &block.related_commits);
    Ok(DiffBlock {
        id: block.id,
        old_start: block.old_start as u64,
        old_end: block.old_end as u64,
        new_start: block.new_start as u64,
        new_end: block.new_end as u64,
        row_start_index: block.row_start_index.map(|value| value as u64),
        row_end_index: block.row_end_index.map(|value| value as u64),
        change_type: adapt_block_change_type(&block.change_type)?,
        authors,
        rows: adapt_rows(block.rows)?,
        related_commits: adapt_related_commits(block.related_commits)?,
        attribution: block.attribution.map(adapt_block_attribution).transpose()?,
    })
}

fn adapt_block_change_type(change_type: &str) -> CommandResult<DiffBlockChangeType> {
    match change_type {
        "added" => Ok(DiffBlockChangeType::Added),
        "deleted" => Ok(DiffBlockChangeType::Deleted),
        "modified" => Ok(DiffBlockChangeType::Modified),
        other => Err(command_error_with_detail(
            "UNKNOWN_DIFF_BLOCK_CHANGE_TYPE",
            format!("未知 diff 块类型：{other}"),
            other,
        )),
    }
}

fn resolved_encoding_code(encoding: ResolvedTextEncoding) -> i16 {
    match encoding {
        ResolvedTextEncoding::Utf8 => 0,
        ResolvedTextEncoding::Gb18030 => 1,
        ResolvedTextEncoding::Utf16Le => 2,
        ResolvedTextEncoding::Utf16Be => 3,
    }
}

fn diff_block_change_type_code(change_type: &str) -> CommandResult<i16> {
    match change_type {
        "added" => Ok(0),
        "deleted" => Ok(1),
        "modified" => Ok(2),
        other => Err(command_error_with_detail(
            "UNKNOWN_DIFF_BLOCK_CHANGE_TYPE",
            format!("未知 diff 块类型：{other}"),
            other,
        )),
    }
}

fn cached_diff_block_change_type(change_type: i16) -> CommandResult<DiffBlockChangeType> {
    match change_type {
        0 => Ok(DiffBlockChangeType::Added),
        1 => Ok(DiffBlockChangeType::Deleted),
        2 => Ok(DiffBlockChangeType::Modified),
        other => Err(command_error_with_detail(
            "UNKNOWN_DIFF_BLOCK_CHANGE_TYPE",
            format!("未知缓存 diff 块类型：{other}"),
            other.to_string(),
        )),
    }
}

fn attribution_confidence_code(confidence: &str) -> CommandResult<i16> {
    match confidence {
        "precise" => Ok(0),
        "inferred" => Ok(1),
        "partial" => Ok(2),
        other => Err(command_error_with_detail(
            "UNKNOWN_ATTRIBUTION_CONFIDENCE",
            format!("未知归因置信度：{other}"),
            other,
        )),
    }
}

fn attribution_method_code(method: &str) -> CommandResult<i16> {
    match method {
        "blame" => Ok(0),
        "merge-trace" => Ok(1),
        "patch-inference" => Ok(2),
        "deletion-trace" => Ok(3),
        other => Err(command_error_with_detail(
            "UNKNOWN_ATTRIBUTION_METHOD",
            format!("未知归因方法：{other}"),
            other,
        )),
    }
}

fn attribution_warning_flags(warnings: &[AttributionWarningOutput]) -> u32 {
    warnings.iter().fold(0, |flags, warning| {
        flags
            | match warning.code.as_str() {
                "BLAME_UNAVAILABLE" => 1,
                "MERGE_TRACE_AMBIGUOUS" => 1 << 1,
                "PATH_HISTORY_INCOMPLETE" => 1 << 2,
                "DELETION_TRACE_INCOMPLETE" => 1 << 3,
                "EOF_NEWLINE_ATTRIBUTION_UNAVAILABLE" => 1 << 4,
                _ => 0,
            }
    })
}

fn cached_block_attribution(
    confidence: i16,
    warning_flags: u32,
) -> CommandResult<BlockAttributionSummary> {
    let confidence = match confidence {
        0 => AttributionConfidence::Precise,
        1 => AttributionConfidence::Inferred,
        2 => AttributionConfidence::Partial,
        other => {
            return Err(command_error_with_detail(
                "UNKNOWN_ATTRIBUTION_CONFIDENCE",
                format!("未知缓存归因置信度：{other}"),
                other.to_string(),
            ))
        }
    };
    let mut warnings = Vec::new();
    for (flag, code, message) in [
        (1, AttributionWarningCode::BlameUnavailable, "Blame 不可用"),
        (
            1 << 1,
            AttributionWarningCode::MergeTraceAmbiguous,
            "合并追踪存在歧义",
        ),
        (
            1 << 2,
            AttributionWarningCode::PathHistoryIncomplete,
            "路径历史不完整",
        ),
        (
            1 << 3,
            AttributionWarningCode::DeletionTraceIncomplete,
            "删除追踪不完整",
        ),
        (
            1 << 4,
            AttributionWarningCode::EofNewlineAttributionUnavailable,
            "文件末尾换行归因不可用",
        ),
    ] {
        if warning_flags & flag != 0 {
            warnings.push(AttributionWarning {
                code,
                message: message.to_string(),
            });
        }
    }
    Ok(BlockAttributionSummary {
        confidence,
        warnings,
    })
}

fn cached_related_attribution(
    method: i16,
    merge_hashes: &[String],
) -> CommandResult<RelatedCommitAttributionOutput> {
    let method = match method {
        0 => "blame",
        1 => "merge-trace",
        2 => "patch-inference",
        3 => "deletion-trace",
        other => {
            return Err(command_error_with_detail(
                "UNKNOWN_ATTRIBUTION_METHOD",
                format!("未知缓存归因方法：{other}"),
                other.to_string(),
            ))
        }
    };
    Ok(RelatedCommitAttributionOutput {
        method: method.to_string(),
        via_merge_hashes: merge_hashes.to_vec(),
    })
}

fn adapt_authors(
    authors: Vec<AuthorOutput>,
    commits: &[RelatedCommitOutput],
) -> Vec<AuthorSummary> {
    let mut seen_identities = HashSet::new();
    authors
        .into_iter()
        .filter_map(|author| {
            let Some(identity) = author_identity(&author.name, author.email.as_deref()) else {
                return Some(AuthorSummary {
                    name: author.name,
                    email: author.email,
                    commit_count: 0,
                    last_committed_at: String::new(),
                });
            };
            if !seen_identities.insert(identity.clone()) {
                return None;
            }

            let related = commits.iter().filter(|commit| {
                author_identity(&commit.author_name, commit.author_email.as_deref()).as_ref()
                    == Some(&identity)
            });
            let mut hashes = HashSet::new();
            let mut latest: Option<&RelatedCommitOutput> = None;
            for commit in related {
                hashes.insert(commit.hash.as_str());
                if latest.is_none_or(|current| commit_is_newer(commit, current)) {
                    latest = Some(commit);
                }
            }

            let (name, email, last_committed_at) = latest.map_or_else(
                || (author.name, author.email, String::new()),
                |commit| {
                    (
                        commit.author_name.clone(),
                        commit.author_email.clone(),
                        commit.committed_at.clone(),
                    )
                },
            );
            Some(AuthorSummary {
                name,
                email,
                commit_count: hashes.len() as u64,
                last_committed_at,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum AuthorIdentity {
    Email(String),
    Name(String),
}

fn author_identity(name: &str, email: Option<&str>) -> Option<AuthorIdentity> {
    if let Some(email) = email.map(str::trim).filter(|email| !email.is_empty()) {
        return Some(AuthorIdentity::Email(email.to_lowercase()));
    }
    let name = name.trim();
    (!name.is_empty()).then(|| AuthorIdentity::Name(name.to_lowercase()))
}

fn commit_is_newer(candidate: &RelatedCommitOutput, current: &RelatedCommitOutput) -> bool {
    compare_commit_times(&candidate.committed_at, &current.committed_at) == Ordering::Greater
}

fn compare_commit_times(candidate: &str, current: &str) -> Ordering {
    match (
        DateTime::parse_from_rfc3339(candidate),
        DateTime::parse_from_rfc3339(current),
    ) {
        (Ok(candidate_time), Ok(current_time)) => candidate_time.cmp(&current_time),
        (Ok(_), Err(_)) => Ordering::Greater,
        (Err(_), Ok(_)) => Ordering::Less,
        (Err(_), Err(_)) => candidate.cmp(current),
    }
}

fn adapt_related_commits(commits: Vec<RelatedCommitOutput>) -> CommandResult<Vec<RelatedCommit>> {
    commits.into_iter().map(adapt_related_commit).collect()
}

fn adapt_related_commit(commit: RelatedCommitOutput) -> CommandResult<RelatedCommit> {
    Ok(RelatedCommit {
        hash: commit.hash,
        short_hash: commit.short_hash,
        author_name: commit.author_name,
        author_email: commit.author_email,
        committed_at: commit.committed_at,
        subject: commit.subject,
        matched_by_filter: commit.matched_by_filter,
        touched_ranges: commit
            .touched_ranges
            .into_iter()
            .map(adapt_touched_range)
            .collect(),
        attribution: commit
            .attribution
            .map(adapt_related_commit_attribution)
            .transpose()?,
    })
}

fn adapt_touched_range(range: TouchedRangeOutput) -> TouchedRange {
    TouchedRange {
        old_start: range.old_start.map(|value| value as u64),
        old_end: range.old_end.map(|value| value as u64),
        new_start: range.new_start.map(|value| value as u64),
        new_end: range.new_end.map(|value| value as u64),
    }
}

fn adapt_block_attribution(
    attribution: BlockAttributionOutput,
) -> CommandResult<BlockAttributionSummary> {
    Ok(BlockAttributionSummary {
        confidence: adapt_attribution_confidence(&attribution.confidence)?,
        warnings: attribution
            .warnings
            .into_iter()
            .map(adapt_attribution_warning)
            .collect::<CommandResult<Vec<_>>>()?,
    })
}

fn adapt_attribution_confidence(confidence: &str) -> CommandResult<AttributionConfidence> {
    match confidence {
        "precise" => Ok(AttributionConfidence::Precise),
        "inferred" => Ok(AttributionConfidence::Inferred),
        "partial" => Ok(AttributionConfidence::Partial),
        other => Err(command_error_with_detail(
            "UNKNOWN_ATTRIBUTION_CONFIDENCE",
            format!("未知归因置信度：{other}"),
            other,
        )),
    }
}

fn adapt_attribution_warning(
    warning: AttributionWarningOutput,
) -> CommandResult<AttributionWarning> {
    Ok(AttributionWarning {
        code: adapt_attribution_warning_code(&warning.code)?,
        message: warning.message,
    })
}

fn adapt_attribution_warning_code(code: &str) -> CommandResult<AttributionWarningCode> {
    match code {
        "BLAME_UNAVAILABLE" => Ok(AttributionWarningCode::BlameUnavailable),
        "MERGE_TRACE_AMBIGUOUS" => Ok(AttributionWarningCode::MergeTraceAmbiguous),
        "PATH_HISTORY_INCOMPLETE" => Ok(AttributionWarningCode::PathHistoryIncomplete),
        "DELETION_TRACE_INCOMPLETE" => Ok(AttributionWarningCode::DeletionTraceIncomplete),
        "EOF_NEWLINE_ATTRIBUTION_UNAVAILABLE" => {
            Ok(AttributionWarningCode::EofNewlineAttributionUnavailable)
        }
        other => Err(command_error_with_detail(
            "UNKNOWN_ATTRIBUTION_WARNING_CODE",
            format!("未知归因警告代码：{other}"),
            other,
        )),
    }
}

fn adapt_related_commit_attribution(
    attribution: RelatedCommitAttributionOutput,
) -> CommandResult<RelatedCommitAttribution> {
    Ok(RelatedCommitAttribution {
        method: adapt_attribution_method(&attribution.method)?,
        via_merge_hashes: attribution.via_merge_hashes,
    })
}

fn adapt_attribution_method(method: &str) -> CommandResult<AttributionMethod> {
    match method {
        "blame" => Ok(AttributionMethod::Blame),
        "merge-trace" => Ok(AttributionMethod::MergeTrace),
        "patch-inference" => Ok(AttributionMethod::PatchInference),
        "deletion-trace" => Ok(AttributionMethod::DeletionTrace),
        other => Err(command_error_with_detail(
            "UNKNOWN_ATTRIBUTION_METHOD",
            format!("未知归因方法：{other}"),
            other,
        )),
    }
}

fn map_analysis_error(error: AnalysisAppError) -> revier_analysis::contracts::AppError {
    let code = match &error {
        AnalysisAppError::InvalidArgument(_) => "INVALID_ARGUMENT",
        AnalysisAppError::Repository(_) => "REPOSITORY_ERROR",
        AnalysisAppError::FileNotAnalyzable(_) => "FILE_NOT_ANALYZABLE",
        AnalysisAppError::IndexUnavailable(_) => "INDEX_UNAVAILABLE",
        AnalysisAppError::RequiredIndexUnavailable(_) => "REQUIRED_INDEX_UNAVAILABLE",
        AnalysisAppError::SchemaIncompatible(_) => "SCHEMA_INCOMPATIBLE",
        AnalysisAppError::CacheInvalid(_) => "CACHE_INVALID",
        AnalysisAppError::DuckDb(_) => "DUCKDB_ERROR",
        AnalysisAppError::Spike(_) => "SPIKE_ERROR",
        AnalysisAppError::Analysis(_) => "ANALYSIS_ERROR",
        AnalysisAppError::Cancelled => "TASK_CANCELLED",
        AnalysisAppError::Json(_) => "JSON_ERROR",
        AnalysisAppError::Io(_) => "IO_ERROR",
    };
    command_error(code, error.to_string())
}

fn status_label(status: &AnalysisTaskStatus) -> &'static str {
    match status {
        AnalysisTaskStatus::Pending => "pending",
        AnalysisTaskStatus::Running => "running",
        AnalysisTaskStatus::Completed => "completed",
        AnalysisTaskStatus::Failed => "failed",
        AnalysisTaskStatus::Cancelled => "cancelled",
    }
}

fn error_detail(error: &revier_analysis::contracts::AppError) -> String {
    match &error.detail {
        Some(detail) => format!("{}: {} ({detail})", error.code, error.message),
        None => format!("{}: {}", error.code, error.message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use revier_analysis::json::FileOverlayCommandOutput;
    use std::ffi::OsString;
    use std::path::Path;
    use std::process::Command;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use revier_analysis::cli::{IndexBuildArgs, IndexCommonArgs, OutputFormat};
    use revier_analysis::contracts::{
        AnalysisStage, AnalysisTaskStatus, ChangedFileStatus, CommitOverlayRequest,
        FileOverlayMode, FileOverlayRequest, ProjectPreferences, ReviewAuthorOptionsRequest,
        ReviewFilters,
    };
    use tempfile::{tempdir, TempDir};

    use crate::services::projects::ProjectService;

    #[test]
    fn throttles_ordinary_progress_and_emits_stage_changes_immediately() {
        let tasks = Arc::new(Mutex::new(HashMap::new()));
        let task = AnalysisTaskSnapshot {
            task_id: "task-progress".to_string(),
            project_id: "project-1".to_string(),
            status: AnalysisTaskStatus::Running,
            stage: AnalysisStage::ReadRepository,
            progress: None,
            message: None,
            error: None,
        };
        tasks
            .lock()
            .expect("任务锁被污染")
            .insert(task.task_id.clone(), task);
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&events);
        let reporter = TaskProgressReporter {
            tasks,
            task_id: "task-progress".to_string(),
            started: Instant::now(),
            registration: Some(TaskProgressRegistration {
                operation_id: "operation-1".to_string(),
                project_id: "project-1".to_string(),
                branch: "main".to_string(),
                started_at: Utc::now().to_rfc3339(),
                started: Instant::now(),
                sink: Arc::new(move |event| {
                    captured.lock().expect("事件锁被污染").push(event);
                }),
            }),
            emission: Mutex::new(ProgressEmissionState::default()),
        };

        reporter.report(OperationProgressUpdate {
            stage: OperationStage::IndexCommits,
            message: "索引 1".to_string(),
            completed_units: Some(1),
            total_units: Some(10),
        });
        reporter.report(OperationProgressUpdate {
            stage: OperationStage::IndexCommits,
            message: "索引 2".to_string(),
            completed_units: Some(2),
            total_units: Some(10),
        });
        reporter.report(OperationProgressUpdate {
            stage: OperationStage::FilterFiles,
            message: "筛选文件".to_string(),
            completed_units: None,
            total_units: None,
        });

        let events = events.lock().expect("事件锁被污染");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].stage, OperationStage::IndexCommits);
        assert_eq!(events[1].stage, OperationStage::FilterFiles);
    }

    #[test]
    fn terminal_progress_releases_registered_event_sink() {
        let service = ReviewService::default();
        let task = service
            .start_analysis_task_with_progress(
                review_filters("project-1".to_string()),
                "operation-1".to_string(),
                Arc::new(|_| {}),
            )
            .expect("创建带进度的任务失败");

        let terminal = service.take_operation_terminal_snapshot(
            &task.task_id,
            OperationStatus::Completed,
            "项目分析完成".to_string(),
        );

        assert_eq!(terminal.expect("应返回终态").progress, Some(1.0));
        assert!(service
            .take_operation_terminal_snapshot(
                &task.task_id,
                OperationStatus::Completed,
                "重复终态".to_string(),
            )
            .is_none());
    }

    #[test]
    fn aggregates_authors_by_normalized_email_and_sorts_source_data() {
        let authors = vec![
            AuthorOutput {
                name: "旧名称".to_string(),
                email: Some(" Alice@Example.COM ".to_string()),
            },
            AuthorOutput {
                name: "未匹配作者".to_string(),
                email: Some("missing@example.com".to_string()),
            },
        ];
        let commits = vec![
            related_commit_output(
                "newest",
                "Alice New",
                Some("alice@example.com"),
                "2026-06-12T00:00:00Z",
            ),
            related_commit_output(
                "oldest",
                "Alice Old",
                Some("ALICE@EXAMPLE.COM"),
                "2026-06-10T00:00:00Z",
            ),
            related_commit_output(
                "newest",
                "Alice Duplicate",
                Some(" alice@example.com "),
                "2026-06-11T00:00:00Z",
            ),
        ];

        let summaries = adapt_authors(authors, &commits);

        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].name, "Alice New");
        assert_eq!(summaries[0].email.as_deref(), Some("alice@example.com"));
        assert_eq!(summaries[0].commit_count, 2);
        assert_eq!(summaries[0].last_committed_at, "2026-06-12T00:00:00Z");
        assert_eq!(summaries[1].name, "未匹配作者");
        assert_eq!(summaries[1].commit_count, 0);
        assert_eq!(summaries[1].last_committed_at, "");
    }

    #[test]
    fn falls_back_to_normalized_name_without_email() {
        let authors = vec![AuthorOutput {
            name: " Alice ".to_string(),
            email: None,
        }];
        let commits = vec![
            related_commit_output("old", "alice", None, "2026-06-10T00:00:00Z"),
            related_commit_output("new", "ALICE", None, "2026-06-11T00:00:00Z"),
        ];

        let summaries = adapt_authors(authors, &commits);

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "ALICE");
        assert_eq!(summaries[0].email, None);
        assert_eq!(summaries[0].commit_count, 2);
        assert_eq!(summaries[0].last_committed_at, "2026-06-11T00:00:00Z");
    }

    #[test]
    fn separates_email_and_name_identities_and_keeps_empty_identities_distinct() {
        let authors = vec![
            AuthorOutput {
                name: "邮箱作者".to_string(),
                email: Some("alice".to_string()),
            },
            AuthorOutput {
                name: "Alice".to_string(),
                email: None,
            },
            AuthorOutput {
                name: " ".to_string(),
                email: Some(" ".to_string()),
            },
            AuthorOutput {
                name: String::new(),
                email: None,
            },
        ];
        let commits = vec![
            related_commit_output("email", "邮箱提交", Some("ALICE"), "2026-06-10T00:00:00Z"),
            related_commit_output("name", "alice", None, "2026-06-11T00:00:00Z"),
            related_commit_output("empty", "", None, "2026-06-12T00:00:00Z"),
        ];

        let summaries = adapt_authors(authors, &commits);

        assert_eq!(summaries.len(), 4);
        assert_eq!(summaries[0].commit_count, 1);
        assert_eq!(summaries[1].commit_count, 1);
        assert_eq!(summaries[2].commit_count, 0);
        assert_eq!(summaries[3].commit_count, 0);
    }

    #[test]
    fn prefers_valid_rfc3339_over_invalid_timestamp() {
        let summaries = adapt_authors(
            vec![AuthorOutput {
                name: "Alice".to_string(),
                email: Some("alice@example.com".to_string()),
            }],
            &[
                related_commit_output(
                    "invalid",
                    "Alice Invalid",
                    Some("alice@example.com"),
                    "not-a-time",
                ),
                related_commit_output(
                    "valid",
                    "Alice Valid",
                    Some("alice@example.com"),
                    "2026-06-10T00:00:00Z",
                ),
            ],
        );

        assert_eq!(summaries[0].name, "Alice Valid");
        assert_eq!(summaries[0].last_committed_at, "2026-06-10T00:00:00Z");
    }

    #[test]
    fn keeps_first_commit_for_equivalent_instants_and_uses_stable_invalid_fallback() {
        let equivalent = adapt_authors(
            vec![AuthorOutput {
                name: "Alice".to_string(),
                email: Some("alice@example.com".to_string()),
            }],
            &[
                related_commit_output(
                    "first",
                    "Alice First",
                    Some("alice@example.com"),
                    "2026-06-10T00:00:00Z",
                ),
                related_commit_output(
                    "second",
                    "Alice Second",
                    Some("alice@example.com"),
                    "2026-06-10T08:00:00+08:00",
                ),
            ],
        );
        let invalid = adapt_authors(
            vec![AuthorOutput {
                name: "Bob".to_string(),
                email: Some("bob@example.com".to_string()),
            }],
            &[
                related_commit_output("z", "Bob Z", Some("bob@example.com"), "zulu"),
                related_commit_output("a", "Bob A", Some("bob@example.com"), "alpha"),
            ],
        );

        assert_eq!(equivalent[0].name, "Alice First");
        assert_eq!(invalid[0].name, "Bob Z");
        assert_eq!(invalid[0].last_committed_at, "zulu");
    }

    #[test]
    fn required_blob_reports_commit_and_path_when_tree_entry_is_missing() {
        let fixture = create_linear_repo();
        let head = git_output(fixture.path(), ["rev-parse", "HEAD"]);
        let repo = revier_analysis::git::repository::open_repository(fixture.path())
            .expect("打开测试仓库失败");

        let error = require_blob_at_commit(
            revier_analysis::git::blob::read_blob_at_commit(&repo, &head, "src/missing.txt"),
            &head,
            "src/missing.txt",
        )
        .expect_err("缺失 Blob 不应伪装为空文本");

        assert_eq!(error.code, "BLOB_NOT_FOUND");
        let detail = error.detail.expect("应包含缺失 Blob 详情");
        assert!(detail.contains(&head));
        assert!(detail.contains("src/missing.txt"));
    }

    #[test]
    fn keeps_full_contents_and_resolved_encoding_when_adapting_overlay() {
        let output = FileOverlayCommandOutput {
            version: 1,
            overlay: revier_analysis::json::FileOverlayOutput {
                mode: "range".to_string(),
                file: ChangedFileOutput {
                    path: "src/app.txt".to_string(),
                    old_path: None,
                    old_blob_id: None,
                    new_blob_id: None,
                    status: "modified".to_string(),
                    additions: 1,
                    deletions: 1,
                    is_binary: false,
                    is_previewable: true,
                },
                range: revier_analysis::json::AnalysisRangeOutput {
                    branch: "ignored".to_string(),
                    base_commit: "ignored-base".to_string(),
                    head_commit: "ignored-head".to_string(),
                    start_at: None,
                    end_at: None,
                },
                old_content: "旧行\r\n末行\r\n".to_string(),
                new_content: "新行\r\n末行\r\n".to_string(),
                resolved_encoding: "gb18030".to_string(),
                rows: Vec::new(),
                blocks: Vec::new(),
                warnings: Vec::new(),
            },
            warnings: Vec::new(),
        };
        let range = AnalysisRange {
            branch: "main".to_string(),
            base_commit: "base".to_string(),
            head_commit: "head".to_string(),
            start_at: None,
            end_at: None,
        };

        let overlay = adapt_file_overlay_output(output, &range).expect("适配 overlay 失败");

        assert_eq!(
            overlay.old_content.as_bytes(),
            "旧行\r\n末行\r\n".as_bytes()
        );
        assert_eq!(
            overlay.new_content.as_bytes(),
            "新行\r\n末行\r\n".as_bytes()
        );
        assert_eq!(
            overlay.resolved_encoding,
            revier_analysis::contracts::ResolvedTextEncoding::Gb18030
        );
    }

    fn related_commit_output(
        hash: &str,
        name: &str,
        email: Option<&str>,
        committed_at: &str,
    ) -> RelatedCommitOutput {
        RelatedCommitOutput {
            hash: hash.to_string(),
            short_hash: hash.to_string(),
            author_name: name.to_string(),
            author_email: email.map(str::to_string),
            committed_at: committed_at.to_string(),
            subject: "测试提交".to_string(),
            matched_by_filter: false,
            touched_ranges: Vec::new(),
            attribution: None,
        }
    }

    #[test]
    fn creates_and_completes_real_analysis_task() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        build_default_index(fixture.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();

        let task = service
            .start_analysis(&projects, review_filters(project.id))
            .expect("执行真实分析失败");

        assert!(matches!(task.status, AnalysisTaskStatus::Completed));
        let files = service
            .list_changed_files(&task.task_id)
            .expect("读取缓存文件失败");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/app.txt");
        assert!(matches!(files[0].status, ChangedFileStatus::Modified));
        assert_eq!(files[0].additions, 1);
        assert_eq!(files[0].deletions, 0);
    }

    #[test]
    fn start_analysis_task_returns_running_snapshot_before_execution() {
        let service = ReviewService::default();

        let task = service
            .start_analysis_task(review_filters("project-1".to_string()))
            .expect("创建后台分析任务失败");

        assert!(matches!(task.status, AnalysisTaskStatus::Running));
        assert!(matches!(task.stage, AnalysisStage::ReadRepository));
        assert_eq!(task.project_id, "project-1");
        assert!(task.message.is_some());
    }

    #[test]
    fn list_changed_files_returns_task_not_found_when_task_missing() {
        let service = ReviewService::default();

        let error = service
            .list_changed_files("missing-task")
            .expect_err("不存在任务不应返回文件列表");

        assert_eq!(error.code, "TASK_NOT_FOUND");
    }

    #[test]
    fn list_changed_files_returns_status_error_when_task_not_completed() {
        let service = ReviewService::default();
        let task = service.create_task("project-1".to_string());

        let error = service
            .list_changed_files(&task.task_id)
            .expect_err("未完成任务不应返回文件列表");

        assert_eq!(error.code, "TASK_NOT_COMPLETED");
    }

    #[test]
    fn operation_progress_updates_task_stage_message_and_ratio() {
        let service = ReviewService::default();
        let task = service
            .start_analysis_task(review_filters("project-1".to_string()))
            .expect("创建分析任务失败");
        let context = service.analysis_context_for_task(&task.task_id);

        context.report_progress(OperationProgressUpdate {
            stage: OperationStage::IndexCommits,
            message: "索引提交 2/4".to_string(),
            completed_units: Some(2),
            total_units: Some(4),
        });

        let snapshot = service.get_task(&task.task_id).expect("读取任务失败");
        assert!(matches!(snapshot.stage, AnalysisStage::LoadCommits));
        assert_eq!(snapshot.progress, Some(0.5));
        assert_eq!(snapshot.message.as_deref(), Some("索引提交 2/4"));
    }

    #[test]
    fn list_changed_files_returns_failed_error_when_task_failed() {
        let service = ReviewService::default();
        let task = service.create_task("project-1".to_string());
        service.mark_failed(
            &task.task_id,
            command_error("INDEX_UNAVAILABLE", "索引文件不存在"),
        );

        let error = service
            .list_changed_files(&task.task_id)
            .expect_err("失败任务不应返回文件列表");

        assert_eq!(error.code, "TASK_FAILED");
        assert!(error
            .detail
            .expect("应包含失败详情")
            .contains("INDEX_UNAVAILABLE"));
    }

    #[test]
    fn list_changed_files_reports_missing_cache_for_completed_task() {
        let service = ReviewService::default();
        let task = service.create_task("project-1".to_string());
        service.mark_completed(&task.task_id);

        let error = service
            .list_changed_files(&task.task_id)
            .expect_err("缺少缓存的完成任务不应返回空列表");

        assert_eq!(error.code, "TASK_FILES_NOT_FOUND");
    }

    #[test]
    fn cancel_analysis_marks_existing_task_cancelled() {
        let service = ReviewService::default();
        let task = service.create_task("project-1".to_string());

        let snapshot = service
            .cancel_analysis(&task.task_id)
            .expect("取消任务失败");

        assert!(matches!(snapshot.status, AnalysisTaskStatus::Cancelled));
    }

    #[test]
    fn completed_state_does_not_override_cancelled_task() {
        let service = ReviewService::default();
        let task = service.create_task("project-1".to_string());

        service
            .cancel_analysis(&task.task_id)
            .expect("取消任务失败");
        service.mark_completed(&task.task_id);

        let snapshot = service.get_task(&task.task_id).expect("读取任务失败");
        assert!(matches!(snapshot.status, AnalysisTaskStatus::Cancelled));
    }

    #[test]
    fn completed_state_cleans_late_cached_results_for_cancelled_task() {
        let service = ReviewService::default();
        let task = service.create_task("project-1".to_string());

        service
            .cancel_analysis(&task.task_id)
            .expect("取消任务失败");
        service
            .files_by_task
            .lock()
            .expect("文件缓存锁被污染")
            .insert(
                task.task_id.clone(),
                vec![ChangedFile {
                    path: "src/app.txt".to_string(),
                    old_path: None,
                    status: ChangedFileStatus::Modified,
                    additions: 1,
                    deletions: 0,
                    is_binary: false,
                    is_previewable: true,
                }],
            );
        service
            .contexts_by_task
            .lock()
            .expect("任务上下文锁被污染")
            .insert(
                task.task_id.clone(),
                ReviewTaskContext {
                    project: ReviewProject {
                        id: "project-1".to_string(),
                        name: "fixture".to_string(),
                        repo_path: "E:/Projects/revier".to_string(),
                        pinned: false,
                        last_opened_at: None,
                        preferences: ProjectPreferences {
                            default_branch: None,
                            default_days: None,
                            default_glob_rules: Vec::new(),
                            review_filters: None,
                        },
                    },
                    filters: review_filters("project-1".to_string()),
                    range: AnalysisRange {
                        branch: "main".to_string(),
                        base_commit: "base".to_string(),
                        head_commit: "head".to_string(),
                        start_at: None,
                        end_at: None,
                    },
                },
            );

        service.mark_completed(&task.task_id);

        let snapshot = service.get_task(&task.task_id).expect("读取任务失败");
        assert!(matches!(snapshot.status, AnalysisTaskStatus::Cancelled));
        assert!(!service
            .files_by_task
            .lock()
            .expect("文件缓存锁被污染")
            .contains_key(&task.task_id));
        assert!(!service
            .contexts_by_task
            .lock()
            .expect("任务上下文锁被污染")
            .contains_key(&task.task_id));
    }

    #[test]
    fn failed_state_does_not_override_cancelled_task() {
        let service = ReviewService::default();
        let task = service.create_task("project-1".to_string());

        service
            .cancel_analysis(&task.task_id)
            .expect("取消任务失败");
        service.mark_failed(
            &task.task_id,
            command_error("INDEX_UNAVAILABLE", "索引文件不存在"),
        );

        let snapshot = service.get_task(&task.task_id).expect("读取任务失败");
        assert!(matches!(snapshot.status, AnalysisTaskStatus::Cancelled));
    }

    #[test]
    fn cancel_analysis_returns_task_not_found_when_missing() {
        let service = ReviewService::default();

        let error = service
            .cancel_analysis("missing-task")
            .expect_err("不存在任务不应取消成功");

        assert_eq!(error.code, "TASK_NOT_FOUND");
    }

    #[test]
    fn start_analysis_builds_index_and_completes_when_index_missing() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();

        let task = service
            .start_analysis(&projects, review_filters(project.id))
            .expect("索引缺失时应自动构建并完成分析");

        assert!(matches!(task.status, AnalysisTaskStatus::Completed));
        let files = service
            .list_changed_files(&task.task_id)
            .expect("读取缓存文件失败");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/app.txt");
        let repo = revier_analysis::git::repository::open_repository(fixture.path())
            .expect("打开仓库失败");
        let identity =
            revier_analysis::git::repository::repository_identity(&repo).expect("读取仓库标识失败");
        let db_path = revier_analysis::index::connection::default_database_path(&identity.repo_id)
            .expect("生成默认索引路径失败");
        assert!(db_path.exists(), "应创建默认索引文件");
        let tasks = service.tasks.lock().expect("任务锁被污染");
        assert_eq!(tasks.len(), 1);
        let task = tasks.values().next().expect("应创建完成任务");
        assert!(matches!(task.status, AnalysisTaskStatus::Completed));
    }

    #[test]
    fn start_analysis_rebuilds_index_and_completes_when_head_is_missing() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        build_default_index(fixture.path());
        write_file(fixture.path(), "src/app.txt", "one\ntwo\nthree\n");
        git_commit_with_author(
            fixture.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-12T00:00:00Z",
            "feat: add third line",
        );
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();

        let task = service
            .start_analysis(&projects, review_filters(project.id))
            .expect("索引存在但 head 缺失时应重建并完成分析");

        assert!(matches!(task.status, AnalysisTaskStatus::Completed));
        let files = service
            .list_changed_files(&task.task_id)
            .expect("读取缓存文件失败");
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, "src/app.txt");
    }

    #[test]
    fn successful_analysis_publishes_branch_snapshot_with_blob_ids() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();

        service
            .start_analysis(&projects, review_filters(project.id))
            .expect("执行项目分析失败");

        let snapshot =
            load_cached_snapshot(&service, fixture.path()).expect("成功分析应发布分支快照");
        assert_eq!(snapshot.branch, "main");
        assert_eq!(snapshot.files.len(), 1);
        assert!(snapshot.files[0].old_blob_id.is_some());
        assert!(snapshot.files[0].new_blob_id.is_some());
    }

    #[test]
    fn restores_branch_snapshot_without_starting_a_new_analysis() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let writer = ReviewService::default();
        writer
            .start_analysis(&projects, review_filters(project.id.clone()))
            .expect("执行项目分析失败");
        writer
            .set_branch_selected_file(&projects, &project.id, "main", Some("src/app.txt"))
            .expect("保存选中文件失败");
        drop(writer);

        let restored_service = ReviewService::default();
        let restored = restored_service
            .restore_branch_analysis(&projects, &project.id, "main")
            .expect("恢复项目分析失败");

        assert!(restored.cache_hit);
        assert!(!restored.stale);
        assert_eq!(restored.cache_state, CacheState::Hit);
        assert_eq!(restored.files.len(), 1);
        assert_eq!(restored.last_selected_path.as_deref(), Some("src/app.txt"));
        eprintln!("分支快照热缓存恢复={}ms", restored.cache_read_elapsed_ms);
        let task = restored.task.expect("恢复结果应包含任务快照");
        assert!(matches!(task.status, AnalysisTaskStatus::Completed));
        assert_eq!(
            restored_service
                .list_changed_files(&task.task_id)
                .expect("读取恢复文件失败")
                .len(),
            1
        );
    }

    #[test]
    fn missing_branch_cache_returns_empty_workspace_without_creating_task() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();

        let restored = service
            .restore_branch_analysis(&projects, &project.id, "main")
            .expect("读取空分支缓存失败");

        assert!(!restored.cache_hit);
        assert_eq!(restored.cache_state, CacheState::Miss);
        assert!(restored.task.is_none());
        assert!(restored.files.is_empty());
        assert!(service.tasks.lock().expect("任务锁被污染").is_empty());
    }

    #[test]
    fn reports_stale_cache_after_branch_head_changes_without_reanalysis() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();
        service
            .start_analysis(&projects, review_filters(project.id.clone()))
            .expect("执行项目分析失败");
        write_file(fixture.path(), "src/app.txt", "one\ntwo\nthree\n");
        git_commit_with_author(
            fixture.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-13T00:00:00Z",
            "feat: 新增第三行",
        );

        let restored = service
            .restore_branch_analysis(&projects, &project.id, "main")
            .expect("恢复过期项目分析失败");

        assert!(restored.cache_hit);
        assert!(restored.stale);
        assert_eq!(restored.cache_state, CacheState::Stale);
        assert_ne!(
            restored.cached_head.as_deref(),
            Some(restored.current_head.as_str())
        );
        let error = service
            .get_file_overlay(FileOverlayRequest {
                task_id: restored.task.expect("过期缓存仍应恢复可浏览任务").task_id,
                file_path: "src/app.txt".to_string(),
                operation_id: "operation-stale-refresh".to_string(),
                cache_mode: CacheMode::Refresh,
                encoding: None,
            })
            .expect_err("过期项目缓存不得执行单文件刷新");
        assert_eq!(error.code, "BRANCH_CACHE_STALE");
    }

    #[test]
    fn failed_or_cancelled_analysis_keeps_previous_branch_snapshot() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();
        let filters = review_filters(project.id);
        service
            .start_analysis(&projects, filters.clone())
            .expect("首次分析失败");
        let original_id = load_cached_snapshot(&service, fixture.path())
            .expect("首次快照应存在")
            .analysis_id;

        let mut invalid = filters.clone();
        invalid.start_at = Some("2026-06-30T00:00:00Z".to_string());
        invalid.end_at = Some("2026-06-01T00:00:00Z".to_string());
        assert!(service.start_analysis(&projects, invalid).is_err());
        assert_eq!(
            load_cached_snapshot(&service, fixture.path())
                .expect("失败后旧快照应保留")
                .analysis_id,
            original_id
        );

        let task = service
            .start_analysis_task(filters.clone())
            .expect("创建待取消任务失败");
        service
            .cancel_analysis(&task.task_id)
            .expect("取消分析失败");
        service
            .execute_analysis_task(&projects, &task.task_id, filters)
            .expect("取消任务应返回任务状态");
        assert_eq!(
            load_cached_snapshot(&service, fixture.path())
                .expect("取消后旧快照应保留")
                .analysis_id,
            original_id
        );
    }

    #[test]
    fn records_initial_and_incremental_project_analysis_elapsed_time() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();
        let filters = review_filters(project.id);

        service
            .start_analysis(&projects, filters.clone())
            .expect("首次项目分析失败");
        let initial = load_cached_snapshot(&service, fixture.path()).expect("首次快照应存在");
        write_file(fixture.path(), "src/app.txt", "one\ntwo\nthree\n");
        git_commit_with_author(
            fixture.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-12T00:00:00Z",
            "feat: add third line",
        );
        service
            .start_analysis(&projects, filters)
            .expect("增量项目分析失败");
        let incremental = load_cached_snapshot(&service, fixture.path()).expect("增量快照应存在");

        eprintln!(
            "首次项目分析={}ms，增量项目刷新={}ms",
            initial.elapsed_ms, incremental.elapsed_ms
        );
        assert_ne!(initial.analysis_id, incremental.analysis_id);
        assert_eq!(incremental.files.len(), 1);
    }

    #[test]
    fn list_authors_returns_sorted_options_with_time_filter() {
        let fixture = create_author_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();

        let authors = service
            .list_authors(
                &projects,
                ReviewAuthorOptionsRequest {
                    project_id: project.id,
                    branch: "main".to_string(),
                    start_at: Some("2026-06-01T00:00:00Z".to_string()),
                    end_at: Some("2026-06-30T23:59:59Z".to_string()),
                },
            )
            .expect("读取作者选项失败");

        assert_eq!(authors.len(), 2);
        assert_eq!(authors[0].key, "alice@example.com");
        assert_eq!(authors[0].commit_count, 1);
        assert_eq!(authors[1].key, "bob@example.com");
        assert_eq!(authors[1].commit_count, 1);
    }

    #[test]
    fn get_file_overlay_returns_cached_task_range_overlay() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        build_default_index(fixture.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();
        let task = service
            .start_analysis(&projects, review_filters(project.id))
            .expect("执行真实分析失败");

        let overlay = service
            .get_file_overlay(FileOverlayRequest {
                task_id: task.task_id,
                file_path: "src/app.txt".to_string(),
                operation_id: "operation-file-1".to_string(),
                cache_mode: revier_analysis::contracts::CacheMode::PreferCache,
                encoding: None,
            })
            .expect("读取文件 overlay 失败");

        assert!(matches!(overlay.mode, Some(FileOverlayMode::Range)));
        assert_eq!(overlay.file.path, "src/app.txt");
        assert_eq!(overlay.range.branch, "main");
        assert!(overlay.rows.is_none());
        assert!(overlay.blocks.is_empty());
        assert_eq!(overlay.old_content, "one\n");
        assert_eq!(overlay.new_content, "one\ntwo\n");
        assert_eq!(overlay.resolved_encoding, ResolvedTextEncoding::Utf8);
    }

    #[test]
    fn get_file_overlay_reads_cached_blob_ids_without_resolving_range_commits() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();
        let task = service
            .start_analysis(&projects, review_filters(project.id))
            .expect("执行真实分析失败");
        {
            let mut contexts = service.contexts_by_task.lock().expect("任务上下文锁被污染");
            let context = contexts.get_mut(&task.task_id).expect("任务上下文应存在");
            context.range.base_commit = "invalid-base-that-must-not-be-read".to_string();
            context.range.head_commit = "invalid-head-that-must-not-be-read".to_string();
        }

        let overlay = service
            .get_file_overlay(FileOverlayRequest {
                task_id: task.task_id,
                file_path: "src/app.txt".to_string(),
                operation_id: "operation-file-fast-path".to_string(),
                cache_mode: revier_analysis::contracts::CacheMode::PreferCache,
                encoding: None,
            })
            .expect("Blob ID 快速路径不应读取范围提交");

        assert_eq!(overlay.old_content, "one\n");
        assert_eq!(overlay.new_content, "one\ntwo\n");
    }

    #[test]
    fn file_attribution_is_persisted_then_restored_by_signature() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();
        let task = service
            .start_analysis(&projects, review_filters(project.id))
            .expect("执行真实分析失败");
        let request = AttributeBlocksRequest {
            task_id: task.task_id,
            file_path: "src/app.txt".to_string(),
            operation_id: "operation-attribution-1".to_string(),
            cache_mode: CacheMode::PreferCache,
            block_signature: "signature-added-line".to_string(),
            resolved_encoding: ResolvedTextEncoding::Utf8,
            blocks: vec![revier_analysis::contracts::DiffBlockRange {
                id: "block-added".to_string(),
                old_start: 0,
                old_end: 0,
                new_start: 2,
                new_end: 2,
                change_type: DiffBlockChangeType::Added,
            }],
        };

        let cold_started = Instant::now();
        let first = service
            .attribute_blocks(request.clone())
            .expect("首次文件归因失败");
        let cold_elapsed = cold_started.elapsed();
        let hot_started = Instant::now();
        let second = service
            .attribute_blocks(request.clone())
            .expect("缓存文件归因失败");
        let hot_elapsed = hot_started.elapsed();
        let refresh_started = Instant::now();
        let refreshed = service
            .attribute_blocks(AttributeBlocksRequest {
                cache_mode: CacheMode::Refresh,
                ..request.clone()
            })
            .expect("刷新文件归因失败");
        let refresh_elapsed = refresh_started.elapsed();
        let after_refresh = service
            .attribute_blocks(request)
            .expect("刷新后读取文件归因缓存失败");

        assert_eq!(first.cache_state, CacheState::Miss);
        assert_eq!(second.cache_state, CacheState::Hit);
        assert_eq!(refreshed.cache_state, CacheState::Refresh);
        assert_eq!(after_refresh.cache_state, CacheState::Hit);
        assert_eq!(second.attributions.len(), 1);
        assert_eq!(second.attributions[0].id, "block-added");
        assert_eq!(second.attributions[0].related_commits.len(), 1);
        assert_eq!(
            second.attributions[0].related_commits[0].hash,
            first.attributions[0].related_commits[0].hash
        );
        eprintln!(
            "文件归因冷缓存={}ms，热缓存={}ms，强制刷新={}ms",
            cold_elapsed.as_millis(),
            hot_elapsed.as_millis(),
            refresh_elapsed.as_millis()
        );
    }

    #[test]
    fn commit_overlay_is_cached_by_file_analysis_commit_and_current_encoding() {
        let fixture = create_linear_repo();
        let head = git_output(fixture.path(), ["rev-parse", "HEAD"]);
        let root = git_output(fixture.path(), ["rev-list", "--max-parents=0", "HEAD"]);
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();
        let task = service
            .start_analysis(&projects, review_filters(project.id))
            .expect("执行真实分析失败");
        service
            .attribute_blocks(AttributeBlocksRequest {
                task_id: task.task_id.clone(),
                file_path: "src/app.txt".to_string(),
                operation_id: "operation-file-before-drilldown".to_string(),
                cache_mode: CacheMode::PreferCache,
                block_signature: "signature-added-line".to_string(),
                resolved_encoding: ResolvedTextEncoding::Utf8,
                blocks: vec![revier_analysis::contracts::DiffBlockRange {
                    id: "block-added".to_string(),
                    old_start: 0,
                    old_end: 0,
                    new_start: 2,
                    new_end: 2,
                    change_type: DiffBlockChangeType::Added,
                }],
            })
            .expect("准备文件分析缓存失败");
        let request = CommitOverlayRequest {
            task_id: task.task_id.clone(),
            file_path: "src/app.txt".to_string(),
            commit_hash: head.clone(),
            operation_id: "operation-commit-cache".to_string(),
            cache_mode: CacheMode::PreferCache,
            encoding: None,
        };

        let cold_started = Instant::now();
        let cold = service
            .get_commit_overlay_with_cache(request.clone())
            .expect("首次提交下钻失败");
        let cold_elapsed = cold_started.elapsed();
        let hot_started = Instant::now();
        let hot = service
            .get_commit_overlay_with_cache(request.clone())
            .expect("提交下钻缓存读取失败");
        let hot_elapsed = hot_started.elapsed();
        assert_eq!(cold.cache_state, CacheState::Miss);
        assert_eq!(hot.cache_state, CacheState::Hit);
        assert_eq!(hot.overlay.old_content, cold.overlay.old_content);
        assert_eq!(hot.overlay.new_content, cold.overlay.new_content);
        assert_eq!(hot.overlay.blocks.len(), cold.overlay.blocks.len());
        assert!(cold.overlay.rows.is_some());
        assert!(hot.overlay.rows.is_none());

        let (_, conn) = cache_connection_for_context(
            &service
                .context_for_task(&task.task_id)
                .expect("读取任务上下文失败"),
            &service.database_registry,
        )
        .expect("打开缓存失败");
        let count: i64 = conn
            .query_row("select count(*) from commit_overlays", [], |row| row.get(0))
            .expect("读取下钻缓存数量失败");
        assert_eq!(count, 1);
        let file_analysis_id: String = conn
            .query_row("select file_analysis_id from file_analyses", [], |row| {
                row.get(0)
            })
            .expect("读取文件分析 ID 失败");
        let cached = revier_analysis::cache::repository::load_commit_overlay(
            &conn,
            &file_analysis_id,
            &head,
        )
        .expect("读取下钻缓存失败")
        .expect("首次下钻应写入缓存");
        assert_eq!(cached.historical_path, "src/app.txt");
        assert!(cached.old_blob_id.is_some());
        assert!(cached.new_blob_id.is_some());
        assert!(!cached.blocks.is_empty());
        drop(conn);

        let gb18030 = service
            .get_commit_overlay_with_cache(CommitOverlayRequest {
                encoding: Some(TextEncoding::Gb18030),
                ..request.clone()
            })
            .expect("切换下钻编码失败");
        assert_eq!(gb18030.cache_state, CacheState::Miss);
        assert_eq!(
            gb18030.overlay.resolved_encoding,
            ResolvedTextEncoding::Gb18030
        );

        let root_result = service
            .get_commit_overlay_with_cache(CommitOverlayRequest {
                commit_hash: root,
                operation_id: "operation-root-commit-cache".to_string(),
                encoding: None,
                ..request.clone()
            })
            .expect("缓存另一提交下钻失败");
        assert_eq!(root_result.cache_state, CacheState::Miss);
        let (_, conn) = cache_connection_for_context(
            &service
                .context_for_task(&task.task_id)
                .expect("读取任务上下文失败"),
            &service.database_registry,
        )
        .expect("打开缓存失败");
        let count: i64 = conn
            .query_row("select count(*) from commit_overlays", [], |row| row.get(0))
            .expect("读取下钻缓存数量失败");
        assert_eq!(count, 2);
        drop(conn);

        {
            let mut contexts = service.contexts_by_task.lock().expect("任务上下文锁被污染");
            contexts
                .get_mut(&task.task_id)
                .expect("任务上下文应存在")
                .range
                .head_commit = "invalid-head-that-cache-hit-must-not-resolve".to_string();
        }
        let cached_after_context_damage = service
            .get_commit_overlay_with_cache(CommitOverlayRequest {
                encoding: Some(TextEncoding::Gb18030),
                ..request
            })
            .expect("热缓存不应重新解析范围历史");
        assert_eq!(cached_after_context_damage.cache_state, CacheState::Hit);
        eprintln!(
            "提交下钻冷缓存={}ms，热缓存={}ms",
            cold_elapsed.as_millis(),
            hot_elapsed.as_millis()
        );
    }

    #[test]
    fn file_operation_progress_uses_one_operation_and_continuous_elapsed_time() {
        let fixture = create_linear_repo();
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();
        let task = service
            .start_analysis(&projects, review_filters(project.id))
            .expect("执行真实分析失败");
        let request = FileOverlayRequest {
            task_id: task.task_id.clone(),
            file_path: "src/app.txt".to_string(),
            operation_id: "operation-file-progress".to_string(),
            cache_mode: CacheMode::Refresh,
            encoding: None,
        };
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&events);
        service
            .start_file_operation(
                &request,
                Arc::new(move |progress| {
                    captured.lock().expect("事件锁被污染").push(progress);
                }),
            )
            .expect("启动文件进度失败");
        service.set_file_operation_kind(&request.operation_id, OperationKind::MonacoDiff);
        service.report_file_operation(
            &request.operation_id,
            OperationStatus::Running,
            OperationStage::ComputeDiff,
            "正在计算差异".to_string(),
            None,
        );
        service.set_file_operation_kind(&request.operation_id, OperationKind::FileAttribution);
        service.report_file_operation(
            &request.operation_id,
            OperationStatus::Completed,
            OperationStage::Ready,
            "文件归因完成".to_string(),
            Some(CacheState::Refresh),
        );

        let events = events.lock().expect("事件锁被污染");
        assert_eq!(events.len(), 3);
        assert!(events
            .iter()
            .all(|event| event.operation_id == request.operation_id));
        assert_eq!(events[0].stage, OperationStage::ReadFileContent);
        assert_eq!(events[1].stage, OperationStage::ComputeDiff);
        assert_eq!(events[1].kind, OperationKind::MonacoDiff);
        assert_eq!(events[2].status, OperationStatus::Completed);
        assert_eq!(events[2].kind, OperationKind::FileAttribution);
        assert_eq!(events[2].progress, Some(1.0));
        assert!(events
            .windows(2)
            .all(|pair| pair[0].elapsed_ms <= pair[1].elapsed_ms));
        drop(events);

        let commit_hash = git_output(fixture.path(), ["rev-parse", "HEAD"]);
        let commit_request = CommitOverlayRequest {
            task_id: task.task_id,
            file_path: "src/app.txt".to_string(),
            commit_hash: commit_hash.clone(),
            operation_id: "operation-commit-progress".to_string(),
            cache_mode: CacheMode::PreferCache,
            encoding: None,
        };
        let commit_events = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&commit_events);
        service
            .start_commit_operation(
                &commit_request,
                Arc::new(move |progress| {
                    captured.lock().expect("事件锁被污染").push(progress);
                }),
            )
            .expect("启动提交下钻进度失败");
        service.report_file_operation(
            &commit_request.operation_id,
            OperationStatus::Completed,
            OperationStage::Ready,
            "已从缓存加载提交".to_string(),
            Some(CacheState::Hit),
        );
        let commit_events = commit_events.lock().expect("事件锁被污染");
        assert_eq!(commit_events.len(), 2);
        assert!(commit_events
            .iter()
            .all(|event| event.kind == OperationKind::CommitOverlay));
        assert!(commit_events
            .iter()
            .all(|event| event.commit_hash.as_deref() == Some(commit_hash.as_str())));
        assert_eq!(commit_events[1].cache_state, CacheState::Hit);
    }

    #[test]
    fn get_commit_overlay_returns_first_parent_overlay_for_range_commit() {
        let fixture = create_linear_repo();
        let head = git_output(fixture.path(), ["rev-parse", "HEAD"]);
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        build_default_index(fixture.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();
        let task = service
            .start_analysis(&projects, review_filters(project.id))
            .expect("执行真实分析失败");

        let overlay = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id: task.task_id,
                file_path: "src/app.txt".to_string(),
                commit_hash: head.clone(),
                operation_id: "commit-overlay-test".to_string(),
                cache_mode: CacheMode::PreferCache,
                encoding: None,
            })
            .expect("读取提交 overlay 失败");

        assert!(matches!(overlay.mode, Some(FileOverlayMode::Commit)));
        let commit = overlay.commit.as_ref().expect("应包含提交信息");
        assert_eq!(commit.hash, head);
        assert!(overlay.parent_hash.is_some());
        assert!(!overlay.blocks.is_empty());
        assert_eq!(overlay.old_content, "one\n");
        assert_eq!(overlay.new_content, "one\ntwo\n");
        assert_eq!(overlay.resolved_encoding, ResolvedTextEncoding::Utf8);
        assert!(overlay.blocks.iter().all(|block| {
            block.authors.len() == 1
                && block.authors[0].commit_count == 1
                && block.authors[0].last_committed_at == commit.committed_at
        }));
    }

    #[test]
    fn get_commit_overlay_handles_added_deleted_and_renamed_empty_sides() {
        let fixture = create_commit_boundary_repo();
        let root = git_output(fixture.path(), ["rev-list", "--max-parents=0", "HEAD"]);
        let commits = git_output(fixture.path(), ["rev-list", "--reverse", "HEAD"])
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let head = commits.last().expect("应包含提交").clone();
        let (service, task_id) = completed_review_service(
            fixture.path(),
            root,
            head,
            vec![
                changed_file("src/added.txt", None, ChangedFileStatus::Added),
                changed_file("src/deleted.txt", None, ChangedFileStatus::Deleted),
                changed_file(
                    "src/renamed.txt",
                    Some("src/old.txt"),
                    ChangedFileStatus::Renamed,
                ),
            ],
        );

        let added = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id: task_id.clone(),
                file_path: "src/added.txt".to_string(),
                commit_hash: commits[1].clone(),
                operation_id: "commit-overlay-added".to_string(),
                cache_mode: CacheMode::PreferCache,
                encoding: None,
            })
            .expect("读取新增文件提交失败");
        let deleted = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id: task_id.clone(),
                file_path: "src/deleted.txt".to_string(),
                commit_hash: commits[2].clone(),
                operation_id: "commit-overlay-deleted".to_string(),
                cache_mode: CacheMode::PreferCache,
                encoding: None,
            })
            .expect("读取删除文件提交失败");
        let renamed = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id,
                file_path: "src/renamed.txt".to_string(),
                commit_hash: commits[3].clone(),
                operation_id: "commit-overlay-renamed".to_string(),
                cache_mode: CacheMode::PreferCache,
                encoding: None,
            })
            .expect("读取重命名文件提交失败");

        assert_eq!(added.old_content, "");
        assert_eq!(added.new_content, "added\n");
        assert_eq!(deleted.old_content, "deleted\n");
        assert_eq!(deleted.new_content, "");
        assert_eq!(renamed.old_content, "rename me\n");
        assert_eq!(renamed.new_content, "rename me\n");
    }

    #[test]
    fn get_commit_overlay_honors_utf16_encoding_and_rejects_conflict_and_binary() {
        let fixture = create_encoding_boundary_repo();
        let root = git_output(fixture.path(), ["rev-list", "--max-parents=0", "HEAD"]);
        let head = git_output(fixture.path(), ["rev-parse", "HEAD"]);
        let (service, task_id) = completed_review_service(
            fixture.path(),
            root,
            head.clone(),
            vec![
                changed_file("src/utf16.txt", None, ChangedFileStatus::Modified),
                changed_file("src/binary.bin", None, ChangedFileStatus::Modified),
            ],
        );

        let utf16 = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id: task_id.clone(),
                file_path: "src/utf16.txt".to_string(),
                commit_hash: head.clone(),
                operation_id: "commit-overlay-utf16".to_string(),
                cache_mode: CacheMode::PreferCache,
                encoding: Some(TextEncoding::Utf16Le),
            })
            .expect("显式 UTF-16LE 应成功");
        let conflict = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id: task_id.clone(),
                file_path: "src/utf16.txt".to_string(),
                commit_hash: head.clone(),
                operation_id: "commit-overlay-conflict".to_string(),
                cache_mode: CacheMode::PreferCache,
                encoding: Some(TextEncoding::Utf16Be),
            })
            .expect_err("冲突 BOM 应失败");
        let binary = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id,
                file_path: "src/binary.bin".to_string(),
                commit_hash: head,
                operation_id: "commit-overlay-binary".to_string(),
                cache_mode: CacheMode::PreferCache,
                encoding: None,
            })
            .expect_err("明显二进制文件应失败");

        assert_eq!(utf16.old_content, "旧\n");
        assert_eq!(utf16.new_content, "新\n");
        assert_eq!(utf16.resolved_encoding, ResolvedTextEncoding::Utf16Le);
        assert_eq!(conflict.code, "FILE_NOT_ANALYZABLE");
        assert_eq!(binary.code, "FILE_NOT_ANALYZABLE");
    }

    #[test]
    fn get_commit_overlay_supports_reachable_root_commit_outside_cached_range() {
        let fixture = create_linear_repo();
        let root = git_output(fixture.path(), ["rev-list", "--max-parents=0", "HEAD"]);
        let app_data_dir = tempdir().expect("创建应用数据目录失败");
        let _env = isolated_app_data(app_data_dir.path());
        build_default_index(fixture.path());
        let projects = ProjectService::new(app_data_dir.path().join("projects.json"));
        let project = projects
            .add_project(
                fixture.path().to_string_lossy().to_string(),
                Some("fixture".to_string()),
            )
            .expect("添加项目失败");
        let service = ReviewService::default();
        let task = service
            .start_analysis(&projects, review_filters(project.id))
            .expect("执行真实分析失败");

        let overlay = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id: task.task_id,
                file_path: "src/app.txt".to_string(),
                commit_hash: root.clone(),
                operation_id: "commit-overlay-root".to_string(),
                cache_mode: CacheMode::PreferCache,
                encoding: None,
            })
            .expect("可从 head 追溯的根提交应返回 overlay");

        assert_eq!(
            overlay.commit.as_ref().map(|commit| &commit.hash),
            Some(&root)
        );
        assert_eq!(overlay.old_content, "");
        assert_eq!(overlay.new_content, "one\n");
        assert!(!overlay.blocks.is_empty());
    }

    #[test]
    fn get_commit_overlay_rejects_commit_not_reachable_from_task_head() {
        let (fixture, unrelated) = create_unrelated_commit_repo();
        let root = git_output(fixture.path(), ["rev-list", "--max-parents=0", "main"]);
        let head = git_output(fixture.path(), ["rev-parse", "main"]);
        let (service, task_id) = completed_review_service(
            fixture.path(),
            root,
            head,
            vec![changed_file(
                "src/app.txt",
                None,
                ChangedFileStatus::Modified,
            )],
        );

        let error = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id,
                file_path: "src/app.txt".to_string(),
                commit_hash: unrelated,
                operation_id: "commit-overlay-unreachable".to_string(),
                cache_mode: CacheMode::PreferCache,
                encoding: None,
            })
            .expect_err("不可从任务 head 追溯的提交不应返回 overlay");

        assert_eq!(error.code, "COMMIT_NOT_REACHABLE");
    }

    #[test]
    fn get_commit_overlay_resolves_path_before_multiple_renames() {
        let (fixture, target, base, head) = create_historical_rename_repo();
        let (service, task_id) = completed_review_service(
            fixture.path(),
            base,
            head,
            vec![changed_file(
                "src/current.txt",
                Some("src/middle.txt"),
                ChangedFileStatus::Renamed,
            )],
        );

        let overlay = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id,
                file_path: "src/current.txt".to_string(),
                commit_hash: target,
                operation_id: "commit-overlay-rename-history".to_string(),
                cache_mode: CacheMode::PreferCache,
                encoding: None,
            })
            .expect("多次重命名前的来源提交应返回 overlay");

        assert_eq!(overlay.old_content, "one\n");
        assert_eq!(overlay.new_content, "one\ntwo\n");
    }

    #[test]
    fn rejects_unknown_changed_file_status() {
        let error = adapt_changed_file(revier_analysis::json::ChangedFileOutput {
            path: "src/app.txt".to_string(),
            old_path: None,
            old_blob_id: None,
            new_blob_id: None,
            status: "copied".to_string(),
            additions: 0,
            deletions: 0,
            is_binary: false,
            is_previewable: true,
        })
        .expect_err("未知文件状态不应转换成功");

        assert_eq!(error.code, "UNKNOWN_CHANGED_FILE_STATUS");
    }

    fn review_filters(project_id: String) -> ReviewFilters {
        ReviewFilters {
            project_id,
            branch: "main".to_string(),
            start_at: Some("1970-01-01T00:00:00Z".to_string()),
            end_at: Some("2999-12-31T23:59:59Z".to_string()),
            author_keys: None,
            author_query: None,
            message_query: None,
            glob_rules: vec!["src/**/*.txt".to_string()],
        }
    }

    fn create_linear_repo() -> TempDir {
        let repo = tempdir().expect("创建临时仓库失败");
        git(repo.path(), ["init", "-b", "main"]);
        write_file(repo.path(), "src/app.txt", "one\n");
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-10T00:00:00Z",
            "feat: initial",
        );
        write_file(repo.path(), "src/app.txt", "one\ntwo\n");
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-11T00:00:00Z",
            "feat: add second line",
        );
        repo
    }

    fn create_unrelated_commit_repo() -> (TempDir, String) {
        let repo = create_linear_repo();
        git(repo.path(), ["checkout", "--orphan", "unrelated"]);
        git(repo.path(), ["rm", "-rf", "."]);
        write_file(repo.path(), "src/app.txt", "unrelated\n");
        git_commit_with_author(
            repo.path(),
            "Unrelated Author",
            "unrelated@example.com",
            "2026-06-12T00:00:00Z",
            "feat: unrelated history",
        );
        let unrelated = git_output(repo.path(), ["rev-parse", "HEAD"]);
        git(repo.path(), ["checkout", "main"]);
        (repo, unrelated)
    }

    fn create_historical_rename_repo() -> (TempDir, String, String, String) {
        let repo = tempdir().expect("创建临时仓库失败");
        git(repo.path(), ["init", "-b", "main"]);
        write_file(repo.path(), "src/old.txt", "one\n");
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-09T00:00:00Z",
            "feat: initial historical path",
        );
        write_file(repo.path(), "src/old.txt", "one\ntwo\n");
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-10T00:00:00Z",
            "feat: update historical path",
        );
        let target = git_output(repo.path(), ["rev-parse", "HEAD"]);
        git(repo.path(), ["mv", "src/old.txt", "src/middle.txt"]);
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-11T00:00:00Z",
            "refactor: first rename",
        );
        let base = git_output(repo.path(), ["rev-parse", "HEAD"]);
        git(repo.path(), ["mv", "src/middle.txt", "src/current.txt"]);
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-12T00:00:00Z",
            "refactor: second rename",
        );
        let head = git_output(repo.path(), ["rev-parse", "HEAD"]);
        (repo, target, base, head)
    }

    fn create_author_repo() -> TempDir {
        let repo = tempdir().expect("创建临时仓库失败");
        git(repo.path(), ["init", "-b", "main"]);
        write_file(repo.path(), "src/app.txt", "one\n");
        git_commit_with_author(
            repo.path(),
            "Alice",
            "alice@example.com",
            "2026-05-10T00:00:00Z",
            "feat: initial",
        );
        write_file(repo.path(), "src/app.txt", "one\ntwo\n");
        git_commit_with_author(
            repo.path(),
            "Bob",
            "bob@example.com",
            "2026-06-10T00:00:00Z",
            "feat: bob update",
        );
        write_file(repo.path(), "src/app.txt", "one\ntwo\nthree\n");
        git_commit_with_author(
            repo.path(),
            "Alice",
            "alice@example.com",
            "2026-06-11T00:00:00Z",
            "feat: alice update",
        );
        repo
    }

    fn create_commit_boundary_repo() -> TempDir {
        let repo = tempdir().expect("创建临时仓库失败");
        git(repo.path(), ["init", "-b", "main"]);
        write_file(repo.path(), "src/deleted.txt", "deleted\n");
        write_file(repo.path(), "src/old.txt", "rename me\n");
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-10T00:00:00Z",
            "feat: initial boundaries",
        );
        write_file(repo.path(), "src/added.txt", "added\n");
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-11T00:00:00Z",
            "feat: add file",
        );
        std::fs::remove_file(repo.path().join("src/deleted.txt")).expect("删除测试文件失败");
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-12T00:00:00Z",
            "feat: delete file",
        );
        git(repo.path(), ["mv", "src/old.txt", "src/renamed.txt"]);
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-13T00:00:00Z",
            "feat: rename file",
        );
        repo
    }

    fn create_encoding_boundary_repo() -> TempDir {
        let repo = tempdir().expect("创建临时仓库失败");
        git(repo.path(), ["init", "-b", "main"]);
        write_bytes(repo.path(), "src/utf16.txt", &utf16_le_bytes("旧\n"));
        write_bytes(repo.path(), "src/binary.bin", &[0, 1, 0, 2, 0, 3]);
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-10T00:00:00Z",
            "feat: initial encodings",
        );
        write_bytes(repo.path(), "src/utf16.txt", &utf16_le_bytes("新\n"));
        write_bytes(repo.path(), "src/binary.bin", &[0, 4, 0, 5, 0, 6]);
        git_commit_with_author(
            repo.path(),
            "Fixture Author",
            "fixture@example.com",
            "2026-06-11T00:00:00Z",
            "feat: update encodings",
        );
        repo
    }

    fn completed_review_service(
        repo_path: &Path,
        base_commit: String,
        head_commit: String,
        files: Vec<ChangedFile>,
    ) -> (ReviewService, TaskId) {
        let service = ReviewService::default();
        let task = service.create_task("project-1".to_string());
        service
            .files_by_task
            .lock()
            .expect("文件缓存锁被污染")
            .insert(task.task_id.clone(), files);
        service
            .contexts_by_task
            .lock()
            .expect("任务上下文锁被污染")
            .insert(
                task.task_id.clone(),
                ReviewTaskContext {
                    project: ReviewProject {
                        id: "project-1".to_string(),
                        name: "fixture".to_string(),
                        repo_path: repo_path.to_string_lossy().to_string(),
                        pinned: false,
                        last_opened_at: None,
                        preferences: ProjectPreferences {
                            default_branch: None,
                            default_days: None,
                            default_glob_rules: Vec::new(),
                            review_filters: None,
                        },
                    },
                    filters: review_filters("project-1".to_string()),
                    range: AnalysisRange {
                        branch: "main".to_string(),
                        base_commit,
                        head_commit,
                        start_at: None,
                        end_at: None,
                    },
                },
            );
        service.mark_completed(&task.task_id);
        (service, task.task_id)
    }

    fn changed_file(path: &str, old_path: Option<&str>, status: ChangedFileStatus) -> ChangedFile {
        ChangedFile {
            path: path.to_string(),
            old_path: old_path.map(str::to_string),
            status,
            additions: 0,
            deletions: 0,
            is_binary: false,
            is_previewable: true,
        }
    }

    fn utf16_le_bytes(text: &str) -> Vec<u8> {
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
        bytes
    }

    fn build_default_index(repo_path: &Path) {
        let repo =
            revier_analysis::git::repository::open_repository(repo_path).expect("打开测试仓库失败");
        let identity =
            revier_analysis::git::repository::repository_identity(&repo).expect("读取仓库标识失败");
        let db_path = revier_analysis::index::connection::default_database_path(&identity.repo_id)
            .expect("生成默认索引路径失败");

        revier_analysis::commands::index_build::run(IndexBuildArgs {
            common: IndexCommonArgs {
                repo: repo_path.to_path_buf(),
                db: Some(db_path),
                format: OutputFormat::Json,
                pretty: false,
            },
            branch: "main".to_string(),
        })
        .expect("构建测试索引失败");
    }

    fn load_cached_snapshot(
        service: &ReviewService,
        repo_path: &Path,
    ) -> Option<revier_analysis::cache::models::CachedAnalysisSnapshot> {
        let repo =
            revier_analysis::git::repository::open_repository(repo_path).expect("打开测试仓库失败");
        let identity =
            revier_analysis::git::repository::repository_identity(&repo).expect("读取仓库标识失败");
        let db_path = revier_analysis::index::connection::default_database_path(&identity.repo_id)
            .expect("生成默认索引路径失败");
        let conn = service
            .database_registry
            .connect(&db_path)
            .expect("打开测试索引失败");
        revier_analysis::cache::repository::load_branch_snapshot(&conn, &identity.repo_id, "main")
            .expect("读取分支快照失败")
    }

    fn write_file(repo_path: &Path, path: &str, content: &str) {
        let full_path = repo_path.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).expect("创建父目录失败");
        }
        std::fs::write(full_path, content).expect("写入测试文件失败");
    }

    fn write_bytes(repo_path: &Path, path: &str, content: &[u8]) {
        let full_path = repo_path.join(path);
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).expect("创建父目录失败");
        }
        std::fs::write(full_path, content).expect("写入测试字节失败");
    }

    fn git<const N: usize>(repo_path: &Path, args: [&str; N]) {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .expect("运行 git 命令失败");

        assert!(
            output.status.success(),
            "git 命令失败：{}\nstdout: {}\nstderr: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn git_output<const N: usize>(repo_path: &Path, args: [&str; N]) -> String {
        let output = Command::new("git")
            .current_dir(repo_path)
            .args(args)
            .output()
            .expect("运行 git 命令失败");

        assert!(
            output.status.success(),
            "git 命令失败：{}\nstdout: {}\nstderr: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn git_commit_with_author(
        repo_path: &Path,
        name: &str,
        email: &str,
        date: &str,
        message: &str,
    ) {
        git(repo_path, ["add", "."]);
        let output = Command::new("git")
            .current_dir(repo_path)
            .env("GIT_AUTHOR_NAME", name)
            .env("GIT_AUTHOR_EMAIL", email)
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_NAME", name)
            .env("GIT_COMMITTER_EMAIL", email)
            .env("GIT_COMMITTER_DATE", date)
            .args(["commit", "-m", message])
            .output()
            .expect("运行 git commit 失败");

        assert!(
            output.status.success(),
            "git commit 失败：{}\nstdout: {}\nstderr: {}",
            message,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn isolated_app_data(path: &Path) -> AppDataEnvGuard {
        let lock = APP_DATA_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let guard = AppDataEnvGuard {
            _lock: lock,
            appdata: std::env::var_os("APPDATA"),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME"),
            home: std::env::var_os("HOME"),
        };
        std::env::set_var("APPDATA", path);
        std::env::set_var("XDG_DATA_HOME", path);
        std::env::set_var("HOME", path);
        guard
    }

    static APP_DATA_ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct AppDataEnvGuard {
        _lock: MutexGuard<'static, ()>,
        appdata: Option<OsString>,
        xdg_data_home: Option<OsString>,
        home: Option<OsString>,
    }

    impl Drop for AppDataEnvGuard {
        fn drop(&mut self) {
            restore_env("APPDATA", self.appdata.as_ref());
            restore_env("XDG_DATA_HOME", self.xdg_data_home.as_ref());
            restore_env("HOME", self.home.as_ref());
        }
    }

    fn restore_env(key: &str, value: Option<&OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }
}

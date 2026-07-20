use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use revier_analysis::api::QueryFilesRequest;
use revier_analysis::cli::{IndexBuildArgs, IndexCommonArgs, OutputFormat, OverlayCommonArgs};
use revier_analysis::contracts::{
    AnalysisRange, AnalysisStage, AnalysisTaskSnapshot, AnalysisTaskStatus, AttributeBlocksRequest,
    AttributeBlocksResult, AttributionConfidence, AttributionMethod, AttributionWarning,
    AttributionWarningCode, AuthorFilterOption, AuthorSummary, BlockAttributionSummary,
    ChangedFile, ChangedFileStatus, CommitOverlayRequest, DiffBlock, DiffBlockAttribution,
    DiffBlockChangeType, FileOverlay, FileOverlayMode, FileOverlayRequest, ProjectId,
    RelatedCommit, RelatedCommitAttribution, ResolvedTextEncoding, ReviewAuthorOptionsRequest,
    ReviewFilters, ReviewProject, SideBySideDiffRow, SideBySideDiffRowType, TaskId, TextEncoding,
    TouchedRange, WordChange,
};
use revier_analysis::error::AppError as AnalysisAppError;
use revier_analysis::execution::AnalysisExecutionContext;
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
    tasks: Mutex<HashMap<TaskId, AnalysisTaskSnapshot>>,
    filters_by_task: Mutex<HashMap<TaskId, ReviewFilters>>,
    files_by_task: Mutex<HashMap<TaskId, Vec<ChangedFile>>>,
    contexts_by_task: Mutex<HashMap<TaskId, ReviewTaskContext>>,
    cancellations_by_task: Mutex<HashMap<TaskId, CancellationToken>>,
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

impl ReviewService {
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
        let file = self.file_for_task(&request.task_id, &request.file_path)?;
        let document = load_range_overlay_document(
            &context,
            &request.file_path,
            request.encoding.unwrap_or(TextEncoding::Auto),
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
        self.file_for_task(&request.task_id, &request.file_path)?;
        let document = load_range_overlay_document(
            &context,
            &request.file_path,
            resolved_encoding_as_requested(request.resolved_encoding),
        )?;
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

        let repo = revier_analysis::git::repository::open_repository(Path::new(
            &context.project.repo_path,
        ))
        .map_err(map_analysis_error)?;
        let attribution_context = revier_analysis::attribution::context::AttributionContext::open(
            &repo,
            &overlay_common_args(&context),
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

        Ok(AttributeBlocksResult {
            resolved_encoding: document.resolved_encoding,
            attributions: adapt_block_attributions(blocks)?,
            warnings,
        })
    }

    pub fn get_commit_overlay(&self, request: CommitOverlayRequest) -> CommandResult<FileOverlay> {
        self.ensure_completed_task(&request.task_id)?;
        let context = self.context_for_task(&request.task_id)?;
        let file = self.file_for_task(&request.task_id, &request.file_path)?;
        let repo = revier_analysis::git::repository::open_repository(Path::new(
            &context.project.repo_path,
        ))
        .map_err(map_analysis_error)?;
        let range_hashes = revier_analysis::git::commits::range_commit_hashes(
            &repo,
            &context.range.base_commit,
            &context.range.head_commit,
        )
        .map_err(map_analysis_error)?;
        if !range_hashes.iter().any(|hash| hash == &request.commit_hash) {
            return Err(command_error_with_detail(
                "COMMIT_NOT_IN_RANGE",
                "该提交不在当前分析范围内",
                request.commit_hash,
            ));
        }

        let commit = revier_analysis::git::commits::get_commit(&repo, &request.commit_hash)
            .map_err(map_analysis_error)?;
        let parent_hash = commit.parents.first().cloned().ok_or_else(|| {
            command_error_with_detail(
                "COMMIT_PARENT_NOT_FOUND",
                "该提交没有父提交，无法生成提交级 overlay",
                request.commit_hash.clone(),
            )
        })?;
        let change = revier_analysis::git::diff::commit_file_changes(&repo, &request.commit_hash)
            .map_err(map_analysis_error)?
            .into_iter()
            .find(|change| change.parent_index == 0 && change_matches_file(change, &file))
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

        Ok(FileOverlay {
            mode: Some(FileOverlayMode::Commit),
            file,
            range: context.range,
            rows: Some(rows),
            blocks,
            warnings: Vec::new(),
            commit: Some(related_commit),
            parent_hash: Some(parent_hash),
            old_content: old_text,
            new_content: new_text,
            resolved_encoding,
        })
    }

    fn run_analysis(
        &self,
        projects: &ProjectService,
        task_id: &str,
        filters: ReviewFilters,
    ) -> CommandResult<AnalysisRunResult> {
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
        ensure_analysis_index(
            &repo_path,
            &range.branch,
            &range.base_commit,
            &range.head_commit,
        )
        .map_err(map_analysis_error)?;

        self.ensure_not_cancelled(task_id)?;
        self.mark_running(task_id, AnalysisStage::LoadChangedFiles, "读取变更文件");
        let output = revier_analysis::api::query_files_with_context(
            QueryFilesRequest {
                repo: repo_path,
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
            &context,
        )
        .map_err(map_analysis_error)?;

        self.ensure_not_cancelled(task_id)?;
        let files = output
            .files
            .into_iter()
            .map(adapt_changed_file)
            .collect::<CommandResult<Vec<_>>>()?;
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
        self.cancellations_by_task
            .lock()
            .expect("取消令牌锁被污染")
            .get(task_id)
            .cloned()
            .map(|token| AnalysisExecutionContext::with_cancel(move || token.is_cancelled()))
            .unwrap_or_else(AnalysisExecutionContext::none)
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
    base: &str,
    head: &str,
) -> Result<(), AnalysisAppError> {
    let repo = revier_analysis::git::repository::open_repository(repo_path)?;
    let identity = revier_analysis::git::repository::repository_identity(&repo)?;
    let db_path = revier_analysis::index::connection::default_database_path(&identity.repo_id)?;
    if db_path.exists() && range_is_indexed(&db_path, base, head)? {
        return Ok(());
    }

    revier_analysis::commands::index_build::run(IndexBuildArgs {
        common: IndexCommonArgs {
            repo: repo_path.to_path_buf(),
            db: Some(db_path),
            format: OutputFormat::Json,
            pretty: false,
        },
        branch: branch.to_string(),
    })?;
    Ok(())
}

fn range_is_indexed(db_path: &Path, base: &str, head: &str) -> Result<bool, AnalysisAppError> {
    let conn = revier_analysis::index::connection::open_database(db_path)?;
    revier_analysis::index::migrations::ensure_compatible_schema(&conn)?;
    let base_indexed = revier_analysis::index::queries::commit_exists(&conn, base)?;
    let head_indexed = revier_analysis::index::queries::commit_exists(&conn, head)?;
    Ok(base_indexed && head_indexed)
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
    change: revier_analysis::git::diff::CommitFileChange,
    old_text: String,
    new_text: String,
    resolved_encoding: ResolvedTextEncoding,
    warnings: Vec<revier_analysis::contracts::AppError>,
}

fn load_range_overlay_document(
    context: &ReviewTaskContext,
    file_path: &str,
    requested_encoding: TextEncoding,
) -> CommandResult<OverlayDocument> {
    let repo =
        revier_analysis::git::repository::open_repository(Path::new(&context.project.repo_path))
            .map_err(map_analysis_error)?;
    let change = revier_analysis::git::diff::changed_file_between(
        &repo,
        &context.range.base_commit,
        &context.range.head_commit,
        file_path,
    )
    .map_err(map_analysis_error)?
    .ok_or_else(|| {
        command_error_with_detail("FILE_NOT_CHANGED", "文件在当前范围内未变更", file_path)
    })?;

    let old_path = old_text_path(&change);
    let new_path = new_text_path(&change);
    let old_bytes = match old_path {
        Some(path) => {
            revier_analysis::git::blob::read_blob_at_commit(&repo, &context.range.base_commit, path)
                .map_err(map_analysis_error)?
                .unwrap_or_default()
        }
        None => Vec::new(),
    };
    let new_bytes = match new_path {
        Some(path) => {
            revier_analysis::git::blob::read_blob_at_commit(&repo, &context.range.head_commit, path)
                .map_err(map_analysis_error)?
                .unwrap_or_default()
        }
        None => Vec::new(),
    };
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

fn change_matches_file(
    change: &revier_analysis::git::diff::CommitFileChange,
    file: &ChangedFile,
) -> bool {
    change.path == file.path
        || change.old_path.as_deref() == Some(file.path.as_str())
        || file.old_path.as_deref().is_some_and(|old_path| {
            change.path == old_path || change.old_path.as_deref() == Some(old_path)
        })
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
                encoding: None,
            })
            .expect("读取新增文件提交失败");
        let deleted = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id: task_id.clone(),
                file_path: "src/deleted.txt".to_string(),
                commit_hash: commits[2].clone(),
                encoding: None,
            })
            .expect("读取删除文件提交失败");
        let renamed = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id,
                file_path: "src/renamed.txt".to_string(),
                commit_hash: commits[3].clone(),
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
                encoding: Some(TextEncoding::Utf16Le),
            })
            .expect("显式 UTF-16LE 应成功");
        let conflict = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id: task_id.clone(),
                file_path: "src/utf16.txt".to_string(),
                commit_hash: head.clone(),
                encoding: Some(TextEncoding::Utf16Be),
            })
            .expect_err("冲突 BOM 应失败");
        let binary = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id,
                file_path: "src/binary.bin".to_string(),
                commit_hash: head,
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
    fn get_commit_overlay_rejects_commit_outside_cached_range() {
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

        let error = service
            .get_commit_overlay(CommitOverlayRequest {
                task_id: task.task_id,
                file_path: "src/app.txt".to_string(),
                commit_hash: root,
                encoding: None,
            })
            .expect_err("范围外提交不应返回 overlay");

        assert_eq!(error.code, "COMMIT_NOT_IN_RANGE");
    }

    #[test]
    fn rejects_unknown_changed_file_status() {
        let error = adapt_changed_file(revier_analysis::json::ChangedFileOutput {
            path: "src/app.txt".to_string(),
            old_path: None,
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

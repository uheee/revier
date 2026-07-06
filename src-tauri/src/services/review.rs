use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use revier_analysis::api::QueryFilesRequest;
use revier_analysis::cli::{FileOverlayArgs, OutputFormat, OverlayCommonArgs};
use revier_analysis::contracts::{
    AnalysisRange, AnalysisStage, AnalysisTaskSnapshot, AnalysisTaskStatus, AttributionConfidence,
    AttributionMethod, AttributionWarning, AttributionWarningCode, AuthorFilterOption,
    AuthorSummary, BlockAttributionSummary, ChangedFile, ChangedFileStatus, CommitOverlayRequest,
    DiffBlock, DiffBlockChangeType, FileOverlay, FileOverlayMode, FileOverlayRequest, ProjectId,
    RelatedCommit, RelatedCommitAttribution, ReviewAuthorOptionsRequest, ReviewFilters,
    ReviewProject, SideBySideDiffRow, SideBySideDiffRowType, TaskId, TouchedRange, WordChange,
};
use revier_analysis::error::AppError as AnalysisAppError;
use revier_analysis::json::{
    AttributionWarningOutput, AuthorOutput, BlockAttributionOutput, ChangedFileOutput,
    DiffBlockOutput, FileOverlayCommandOutput, RelatedCommitAttributionOutput, RelatedCommitOutput,
    SideBySideDiffRowOutput, TouchedRangeOutput, WordChangeOutput,
};

use crate::error::{command_error, command_error_with_detail, CommandResult};
use crate::services::projects::ProjectService;

#[derive(Default)]
pub struct ReviewService {
    tasks: Mutex<HashMap<TaskId, AnalysisTaskSnapshot>>,
    filters_by_task: Mutex<HashMap<TaskId, ReviewFilters>>,
    files_by_task: Mutex<HashMap<TaskId, Vec<ChangedFile>>>,
    contexts_by_task: Mutex<HashMap<TaskId, ReviewTaskContext>>,
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
            task.status = AnalysisTaskStatus::Running;
            task.stage = stage;
            task.progress = None;
            task.message = Some(message.to_string());
            task.error = None;
        }
    }

    pub fn mark_completed(&self, task_id: &str) {
        if let Some(task) = self.tasks.lock().expect("任务锁被污染").get_mut(task_id) {
            task.status = AnalysisTaskStatus::Completed;
            task.stage = AnalysisStage::Ready;
            task.progress = Some(1.0);
            task.message = None;
            task.error = None;
        }
    }

    pub fn start_analysis(
        &self,
        projects: &ProjectService,
        filters: ReviewFilters,
    ) -> CommandResult<AnalysisTaskSnapshot> {
        let task = self.create_task(filters.project_id.clone());
        self.filters_by_task
            .lock()
            .expect("筛选条件锁被污染")
            .insert(task.task_id.clone(), filters.clone());

        match self.run_analysis(projects, &task.task_id, filters) {
            Ok(result) => {
                self.files_by_task
                    .lock()
                    .expect("文件缓存锁被污染")
                    .insert(task.task_id.clone(), result.files);
                self.contexts_by_task
                    .lock()
                    .expect("任务上下文锁被污染")
                    .insert(task.task_id.clone(), result.context);
                self.mark_completed(&task.task_id);
                self.get_task(&task.task_id)
            }
            Err(error) => {
                self.mark_failed(&task.task_id, error.clone());
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

    pub fn cancel_analysis(&self, task_id: &str) -> CommandResult<()> {
        // TODO(Task 6 follow-up): 真实在途取消需要后台任务 runner 和取消令牌。
        let mut tasks = self.tasks.lock().expect("任务锁被污染");
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| command_error("TASK_NOT_FOUND", format!("未找到任务：{task_id}")))?;
        task.status = AnalysisTaskStatus::Cancelled;
        task.stage = AnalysisStage::Ready;
        task.progress = None;
        task.message = Some("任务已取消".to_string());
        task.error = None;
        Ok(())
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
        self.file_for_task(&request.task_id, &request.file_path)?;
        let output = revier_analysis::api::file_overlay(FileOverlayArgs {
            common: overlay_common_args(&context),
            file: request.file_path,
        })
        .map_err(map_analysis_error)?;

        adapt_file_overlay_output(output, &context.range)
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
        if change.is_binary {
            return Err(command_error_with_detail(
                "FILE_NOT_ANALYZABLE",
                "文件包含二进制内容",
                change.path,
            ));
        }

        let old_text = match old_text_path(&change) {
            Some(path) => {
                revier_analysis::git::blob::read_text_at_commit(&repo, &parent_hash, path)
                    .map_err(map_analysis_error)?
            }
            None => String::new(),
        };
        let new_text = match new_text_path(&change) {
            Some(path) => {
                revier_analysis::git::blob::read_text_at_commit(&repo, &request.commit_hash, path)
                    .map_err(map_analysis_error)?
            }
            None => String::new(),
        };
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
        })
    }

    fn run_analysis(
        &self,
        projects: &ProjectService,
        task_id: &str,
        filters: ReviewFilters,
    ) -> CommandResult<AnalysisRunResult> {
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

        self.mark_running(task_id, AnalysisStage::ResolveRange, "解析分析范围");
        let range = revier_analysis::api::resolve_analysis_range(
            &repo_path,
            &filters.branch,
            filters.start_at.clone(),
            filters.end_at.clone(),
        )
        .map_err(map_analysis_error)?;

        self.mark_running(task_id, AnalysisStage::LoadChangedFiles, "读取变更文件");
        let output = revier_analysis::api::query_files(QueryFilesRequest {
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
        })
        .map_err(map_analysis_error)?;

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
            task.status = AnalysisTaskStatus::Failed;
            task.error = Some(error.clone());
            task.message = Some(error.message);
        }
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

fn adapt_file_overlay_output(
    output: FileOverlayCommandOutput,
    cached_range: &AnalysisRange,
) -> CommandResult<FileOverlay> {
    let overlay = output.overlay;
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
    })
}

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

fn adapt_block(block: DiffBlockOutput) -> CommandResult<DiffBlock> {
    Ok(DiffBlock {
        id: block.id,
        old_start: block.old_start as u64,
        old_end: block.old_end as u64,
        new_start: block.new_start as u64,
        new_end: block.new_end as u64,
        row_start_index: block.row_start_index.map(|value| value as u64),
        row_end_index: block.row_end_index.map(|value| value as u64),
        change_type: adapt_block_change_type(&block.change_type)?,
        authors: adapt_authors(block.authors),
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

fn adapt_authors(authors: Vec<AuthorOutput>) -> Vec<AuthorSummary> {
    authors
        .into_iter()
        .map(|author| AuthorSummary {
            name: author.name,
            email: author.email,
        })
        .collect()
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
    use std::ffi::OsString;
    use std::path::Path;
    use std::process::Command;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    use revier_analysis::cli::{IndexBuildArgs, IndexCommonArgs, OutputFormat};
    use revier_analysis::contracts::{
        AnalysisTaskStatus, ChangedFileStatus, CommitOverlayRequest, FileOverlayMode,
        FileOverlayRequest, ReviewAuthorOptionsRequest, ReviewFilters,
    };
    use tempfile::{tempdir, TempDir};

    use crate::services::projects::ProjectService;

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

        service
            .cancel_analysis(&task.task_id)
            .expect("取消任务失败");

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
    fn start_analysis_returns_error_and_does_not_complete_when_index_missing() {
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

        let error = service
            .start_analysis(&projects, review_filters(project.id))
            .expect_err("索引缺失时不应完成分析");

        assert_eq!(error.code, "INDEX_UNAVAILABLE");
        let tasks = service.tasks.lock().expect("任务锁被污染");
        assert_eq!(tasks.len(), 1);
        let task = tasks.values().next().expect("应创建失败任务");
        assert!(!matches!(task.status, AnalysisTaskStatus::Completed));
        assert!(matches!(task.status, AnalysisTaskStatus::Failed));
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
            })
            .expect("读取文件 overlay 失败");

        assert!(matches!(overlay.mode, Some(FileOverlayMode::Range)));
        assert_eq!(overlay.file.path, "src/app.txt");
        assert_eq!(overlay.range.branch, "main");
        assert!(!overlay.blocks.is_empty());
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
            })
            .expect("读取提交 overlay 失败");

        assert!(matches!(overlay.mode, Some(FileOverlayMode::Commit)));
        assert_eq!(overlay.commit.as_ref().expect("应包含提交信息").hash, head);
        assert!(overlay.parent_hash.is_some());
        assert!(!overlay.blocks.is_empty());
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

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use revier_analysis::cli::{IndexCommonArgs, OutputFormat, QueryFilesArgs};
use revier_analysis::contracts::{
    AnalysisStage, AnalysisTaskSnapshot, AnalysisTaskStatus, ChangedFile, ChangedFileStatus,
    ProjectId, ReviewFilters, TaskId,
};
use revier_analysis::error::AppError as AnalysisAppError;
use revier_analysis::json::ChangedFileOutput;

use crate::error::{command_error, command_error_with_detail, CommandResult};
use crate::services::projects::ProjectService;

#[derive(Default)]
pub struct ReviewService {
    tasks: Mutex<HashMap<TaskId, AnalysisTaskSnapshot>>,
    filters_by_task: Mutex<HashMap<TaskId, ReviewFilters>>,
    files_by_task: Mutex<HashMap<TaskId, Vec<ChangedFile>>>,
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
            Ok(files) => {
                self.files_by_task
                    .lock()
                    .expect("文件缓存锁被污染")
                    .insert(task.task_id.clone(), files);
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
        let task = self.get_task(task_id)?;
        match &task.status {
            AnalysisTaskStatus::Completed => {}
            AnalysisTaskStatus::Failed => {
                return Err(command_error_with_detail(
                    "TASK_FAILED",
                    format!("任务执行失败：{task_id}"),
                    task.error
                        .as_ref()
                        .map(error_detail)
                        .unwrap_or_else(|| "任务失败原因未知".to_string()),
                ));
            }
            status => {
                return Err(command_error_with_detail(
                    "TASK_NOT_COMPLETED",
                    format!("任务尚未完成：{task_id}"),
                    status_label(status),
                ));
            }
        }

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

    fn run_analysis(
        &self,
        projects: &ProjectService,
        task_id: &str,
        filters: ReviewFilters,
    ) -> CommandResult<Vec<ChangedFile>> {
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
        // TODO(Task 6): 当 revier-analysis 暴露应用层 query_files 参数后，删除这里对 CLI QueryFilesArgs 的兼容适配。
        let output = revier_analysis::api::query_files(QueryFilesArgs {
            common: IndexCommonArgs {
                repo: repo_path,
                db: None,
                format: OutputFormat::Json,
                pretty: false,
            },
            base: range.base_commit,
            head: range.head_commit,
            branch: range.branch,
            authors: filters.author_keys.unwrap_or_default(),
            author_query: filters.author_query,
            message: filters.message_query,
            since: range.start_at,
            until: range.end_at,
            globs: filters.glob_rules,
        })
        .map_err(map_analysis_error)?;

        output.files.into_iter().map(adapt_changed_file).collect()
    }

    fn mark_failed(&self, task_id: &str, error: revier_analysis::contracts::AppError) {
        if let Some(task) = self.tasks.lock().expect("任务锁被污染").get_mut(task_id) {
            task.status = AnalysisTaskStatus::Failed;
            task.error = Some(error.clone());
            task.message = Some(error.message);
        }
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
    use revier_analysis::contracts::{AnalysisTaskStatus, ChangedFileStatus, ReviewFilters};
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
            start_at: None,
            end_at: None,
            author_keys: None,
            author_query: None,
            message_query: None,
            glob_rules: vec!["src/**/*.txt".to_string()],
        }
    }

    fn create_linear_repo() -> TempDir {
        let repo = tempdir().expect("创建临时仓库失败");
        git(repo.path(), ["init", "-b", "main"]);
        git(repo.path(), ["config", "user.name", "Fixture Author"]);
        git(repo.path(), ["config", "user.email", "fixture@example.com"]);
        write_file(repo.path(), "src/app.txt", "one\n");
        git(repo.path(), ["add", "."]);
        git(repo.path(), ["commit", "-m", "feat: initial"]);
        write_file(repo.path(), "src/app.txt", "one\ntwo\n");
        git(repo.path(), ["add", "."]);
        git(repo.path(), ["commit", "-m", "feat: add second line"]);
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

    fn isolated_app_data(path: &Path) -> AppDataEnvGuard {
        let lock = APP_DATA_ENV_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("应用数据环境变量锁被污染");
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

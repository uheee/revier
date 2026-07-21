use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use revier_analysis::contracts::{
    GitBranch, ProjectPreferences, RepositoryValidation, ReviewProject,
};
use revier_analysis::error::AppError as AnalysisAppError;
use serde::{Deserialize, Serialize};

use crate::error::{command_error, command_error_with_detail, CommandResult};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectStoreFile {
    projects: Vec<ReviewProject>,
}

pub struct ProjectService {
    file_path: PathBuf,
    store_lock: Mutex<()>,
}

#[allow(dead_code)]
impl ProjectService {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            store_lock: Mutex::new(()),
        }
    }

    pub fn list_projects(&self) -> CommandResult<Vec<ReviewProject>> {
        let _guard = self.store_lock.lock().expect("项目存储锁被污染");
        Ok(self.read_store()?.projects)
    }

    pub fn get_project(&self, project_id: &str) -> CommandResult<ReviewProject> {
        let _guard = self.store_lock.lock().expect("项目存储锁被污染");
        self.read_store()?
            .projects
            .into_iter()
            .find(|project| project.id == project_id)
            .ok_or_else(|| command_error("PROJECT_NOT_FOUND", format!("未找到项目：{project_id}")))
    }

    pub fn add_project(
        &self,
        repo_path: impl Into<String>,
        name: Option<String>,
    ) -> CommandResult<ReviewProject> {
        self.add_project_with_default_branch(repo_path, name, None)
    }

    pub fn add_project_with_default_branch(
        &self,
        repo_path: impl Into<String>,
        name: Option<String>,
        default_branch: Option<String>,
    ) -> CommandResult<ReviewProject> {
        let _guard = self.store_lock.lock().expect("项目存储锁被污染");
        let mut store = self.read_store()?;
        let repo_path = normalize_path(repo_path.into());
        if store
            .projects
            .iter()
            .any(|project| project.repo_path == repo_path)
        {
            return Err(command_error(
                "PROJECT_ALREADY_EXISTS",
                "该仓库已在项目列表中",
            ));
        }

        let project = ReviewProject {
            id: uuid::Uuid::new_v4().to_string(),
            name: name
                .and_then(|value| {
                    let trimmed = value.trim().to_string();
                    (!trimmed.is_empty()).then_some(trimmed)
                })
                .unwrap_or_else(|| default_project_name(&repo_path)),
            repo_path,
            pinned: false,
            last_opened_at: Some(chrono::Utc::now().to_rfc3339()),
            preferences: ProjectPreferences {
                default_branch,
                default_days: Some(30),
                default_glob_rules: Vec::new(),
                review_filters: None,
            },
        };
        store.projects.push(project.clone());
        self.write_store(&store)?;
        Ok(project)
    }

    pub fn update_project(&self, project: ReviewProject) -> CommandResult<ReviewProject> {
        let _guard = self.store_lock.lock().expect("项目存储锁被污染");
        let mut store = self.read_store()?;
        let index = store
            .projects
            .iter()
            .position(|item| item.id == project.id)
            .ok_or_else(|| {
                command_error("PROJECT_NOT_FOUND", format!("未找到项目：{}", project.id))
            })?;
        let mut updated = project;
        updated.repo_path = normalize_path(updated.repo_path);
        if store
            .projects
            .iter()
            .any(|item| item.id != updated.id && item.repo_path == updated.repo_path)
        {
            return Err(command_error(
                "PROJECT_ALREADY_EXISTS",
                "该仓库已在项目列表中",
            ));
        }

        store.projects[index] = updated.clone();
        self.write_store(&store)?;
        Ok(updated)
    }

    pub fn remove_project(&self, project_id: &str) -> CommandResult<()> {
        let _guard = self.store_lock.lock().expect("项目存储锁被污染");
        let mut store = self.read_store()?;
        let index = store
            .projects
            .iter()
            .position(|project| project.id == project_id)
            .ok_or_else(|| {
                command_error("PROJECT_NOT_FOUND", format!("未找到项目：{project_id}"))
            })?;
        store.projects.remove(index);
        self.write_store(&store)
    }

    pub fn validate_repository(
        &self,
        repo_path: impl AsRef<Path>,
    ) -> CommandResult<RepositoryValidation> {
        revier_analysis::api::validate_repository(repo_path.as_ref()).map_err(map_analysis_error)
    }

    pub fn list_branches(&self, project_id: &str) -> CommandResult<Vec<GitBranch>> {
        let project = self.get_project(project_id)?;
        revier_analysis::api::list_branches(Path::new(&project.repo_path))
            .map_err(map_analysis_error)
    }

    fn read_store(&self) -> CommandResult<ProjectStoreFile> {
        if !self.file_path.exists() {
            return Ok(ProjectStoreFile {
                projects: Vec::new(),
            });
        }
        let content = fs::read_to_string(&self.file_path)
            .map_err(|error| command_error("PROJECT_STORE_READ_FAILED", error.to_string()))?;
        serde_json::from_str(&content)
            .map_err(|error| command_error("PROJECT_STORE_PARSE_FAILED", error.to_string()))
    }

    fn write_store(&self, store: &ProjectStoreFile) -> CommandResult<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| command_error("PROJECT_STORE_WRITE_FAILED", error.to_string()))?;
        }
        let content = serde_json::to_string_pretty(store)
            .map_err(|error| command_error("PROJECT_STORE_SERIALIZE_FAILED", error.to_string()))?;
        fs::write(&self.file_path, format!("{content}\n"))
            .map_err(|error| command_error("PROJECT_STORE_WRITE_FAILED", error.to_string()))
    }
}

#[allow(dead_code)]
fn normalize_path(path: String) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_string()
}

#[allow(dead_code)]
fn default_project_name(repo_path: &str) -> String {
    Path::new(repo_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("未命名仓库")
        .to_string()
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
    command_error_with_detail(code, "分析库调用失败", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Command;
    use tempfile::tempdir;

    #[test]
    fn adds_and_lists_projects() {
        let dir = tempdir().expect("创建临时目录失败");
        let store_path = dir.path().join("projects.json");
        let service = ProjectService::new(store_path);

        let project = service
            .add_project("E:/repo/example", Some("Example".to_string()))
            .expect("添加项目失败");

        let projects = service.list_projects().expect("读取项目失败");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, project.id);
        assert_eq!(projects[0].name, "Example");
        assert_eq!(projects[0].repo_path, "E:/repo/example");
    }

    #[test]
    fn updates_and_removes_projects() {
        let dir = tempdir().expect("创建临时目录失败");
        let service = ProjectService::new(dir.path().join("projects.json"));
        let mut project = service
            .add_project("E:/repo/example", Some("Example".to_string()))
            .expect("添加项目失败");

        project.name = "Updated".to_string();
        project.pinned = true;
        let updated = service
            .update_project(project.clone())
            .expect("更新项目失败");
        assert_eq!(updated.name, "Updated");
        assert!(updated.pinned);
        assert_eq!(
            service
                .get_project(&project.id)
                .expect("读取更新后的项目失败")
                .name,
            "Updated"
        );

        service.remove_project(&project.id).expect("删除项目失败");
        assert!(
            service
                .get_project(&project.id)
                .expect_err("删除后的项目不应存在")
                .code
                == "PROJECT_NOT_FOUND"
        );
    }

    #[test]
    fn validates_repository_and_lists_project_branches() {
        let repo = create_branch_repo();
        let dir = tempdir().expect("创建临时目录失败");
        let service = ProjectService::new(dir.path().join("projects.json"));

        let validation = service
            .validate_repository(repo.path())
            .expect("验证仓库失败");
        assert!(validation.valid);
        assert_eq!(validation.current_branch.as_deref(), Some("main"));

        let project = service
            .add_project_with_default_branch(
                validation.repo_path,
                Some("fixture".to_string()),
                validation.current_branch,
            )
            .expect("添加验证后的项目失败");
        assert_eq!(project.preferences.default_branch.as_deref(), Some("main"));

        let branches = service.list_branches(&project.id).expect("读取分支失败");
        assert!(branches
            .iter()
            .any(|branch| branch.name == "main" && branch.current));
        assert!(branches.iter().any(|branch| branch.name == "feature"));
    }

    #[test]
    fn validate_repository_reports_invalid_directory() {
        let dir = tempdir().expect("创建临时目录失败");
        let service = ProjectService::new(dir.path().join("projects.json"));

        let validation = service
            .validate_repository(dir.path())
            .expect("无效仓库应返回业务结果");

        assert!(!validation.valid);
        assert_eq!(validation.error.as_deref(), Some("请选择一个 Git 仓库目录"));
    }

    fn create_branch_repo() -> tempfile::TempDir {
        let repo = tempdir().expect("创建临时仓库失败");
        git(repo.path(), ["init", "-b", "main"]);
        git(repo.path(), ["config", "user.name", "Fixture Author"]);
        git(repo.path(), ["config", "user.email", "fixture@example.com"]);
        write_file(repo.path(), "README.md", "main\n");
        git(repo.path(), ["add", "."]);
        git(repo.path(), ["commit", "-m", "feat: initial"]);
        git(repo.path(), ["branch", "feature"]);
        repo
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
}

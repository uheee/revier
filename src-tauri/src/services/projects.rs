use std::fs;
use std::path::{Path, PathBuf};

use revier_analysis::contracts::{ProjectPreferences, ReviewProject};
use serde::{Deserialize, Serialize};

use crate::error::{command_error, CommandResult};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectStoreFile {
    projects: Vec<ReviewProject>,
}

pub struct ProjectService {
    file_path: PathBuf,
}

#[allow(dead_code)]
impl ProjectService {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    pub fn list_projects(&self) -> CommandResult<Vec<ReviewProject>> {
        Ok(self.read_store()?.projects)
    }

    pub fn add_project(
        &self,
        repo_path: impl Into<String>,
        name: Option<String>,
    ) -> CommandResult<ReviewProject> {
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
                default_branch: None,
                default_days: Some(30),
                default_glob_rules: Vec::new(),
                review_filters: None,
            },
        };
        store.projects.push(project.clone());
        self.write_store(&store)?;
        Ok(project)
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

#[cfg(test)]
mod tests {
    use super::*;
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
}

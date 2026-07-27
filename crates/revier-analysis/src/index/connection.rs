use crate::error::AppError;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Default)]
pub struct DatabaseRegistry {
    roots: Mutex<HashMap<PathBuf, duckdb::Connection>>,
}

impl DatabaseRegistry {
    pub fn connect(&self, path: &Path) -> Result<duckdb::Connection, AppError> {
        let path = absolute_database_path(path)?;
        let mut roots = self.roots.lock().expect("数据库注册表锁被污染");
        if let Some(root) = roots.get(&path) {
            return root.try_clone().map_err(|error| AppError::DuckDb(error));
        }

        let root = open_database(&path)?;
        let connection = root.try_clone().map_err(|error| AppError::DuckDb(error))?;
        roots.insert(path, root);
        Ok(connection)
    }
}

pub fn open_database(path: &Path) -> Result<duckdb::Connection, AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(AppError::Io)?;
    }
    duckdb::Connection::open(path).map_err(|error| AppError::DuckDb(error))
}

pub fn default_database_path(repo_id: &str) -> Result<PathBuf, AppError> {
    let base = default_app_data_dir()?;
    Ok(base
        .join("revier")
        .join("indexes")
        .join(format!("{repo_id}.duckdb")))
}

fn absolute_database_path(path: &Path) -> Result<PathBuf, AppError> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    std::env::current_dir()
        .map(|current_dir| current_dir.join(path))
        .map_err(AppError::Io)
}

fn default_app_data_dir() -> Result<PathBuf, AppError> {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| AppError::IndexUnavailable("无法读取 APPDATA 环境变量".to_string()))
    } else if cfg!(target_os = "macos") {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| AppError::IndexUnavailable("无法读取 HOME 环境变量".to_string()))?;
        Ok(home.join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local").join("share"))
            })
            .ok_or_else(|| {
                AppError::IndexUnavailable("无法读取 XDG_DATA_HOME 或 HOME 环境变量".to_string())
            })
    }
}

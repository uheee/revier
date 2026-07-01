use crate::error::AppError;
use std::path::{Path, PathBuf};

pub fn open_database(path: &Path) -> Result<duckdb::Connection, AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| AppError::DuckDb(error.to_string()))?;
    }
    duckdb::Connection::open(path).map_err(|error| AppError::DuckDb(error.to_string()))
}

pub fn default_database_path(repo_id: &str) -> Result<PathBuf, AppError> {
    let base = default_app_data_dir()?;
    Ok(base
        .join("revier")
        .join("indexes")
        .join(format!("{repo_id}.duckdb")))
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

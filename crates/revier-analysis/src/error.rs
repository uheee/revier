use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("参数错误：{0}")]
    InvalidArgument(String),

    #[error("仓库读取失败：{0}")]
    Repository(String),

    #[error("文件不可分析：{0}")]
    FileNotAnalyzable(String),

    #[error("索引不可用：{0}")]
    IndexUnavailable(String),

    #[error("索引不可用且命令要求必须使用索引：{0}")]
    RequiredIndexUnavailable(String),

    #[error("索引 schema 不兼容：{0}")]
    SchemaIncompatible(String),

    #[error("缓存已失效：{0}")]
    CacheInvalid(String),

    #[error("DuckDB 读写失败：{0}")]
    DuckDb(#[from] duckdb::Error),

    #[error("gix 能力验证失败：{0}")]
    Spike(String),

    #[error("分析内部错误：{0}")]
    Analysis(String),

    #[error("操作已取消")]
    Cancelled,

    #[error("JSON 输出失败：{0}")]
    Json(#[from] serde_json::Error),

    #[error("文件读写失败：{0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn duckdb_错误保留_source_且退出码不变() {
        let inner = duckdb::Error::InvalidParameterName("missing".into());
        let error = AppError::DuckDb(inner);

        assert!(error.source().is_some());
        assert_eq!(error.exit_code(), 6);
    }

    #[test]
    fn json_和_io_错误保留_source_且退出码不变() {
        let json_error: serde_json::Error =
            serde_json::from_str::<serde_json::Value>("{").expect_err("无效 JSON 应失败");
        let json_error = AppError::Json(json_error);
        assert!(json_error.source().is_some());
        assert_eq!(json_error.exit_code(), 10);

        let io_error = AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"));
        assert!(io_error.source().is_some());
        assert_eq!(io_error.exit_code(), 10);
    }
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidArgument(_) => 2,
            Self::Repository(_) => 3,
            Self::FileNotAnalyzable(_) | Self::IndexUnavailable(_) => 4,
            Self::RequiredIndexUnavailable(_)
            | Self::SchemaIncompatible(_)
            | Self::CacheInvalid(_) => 5,
            Self::DuckDb(_) => 6,
            Self::Cancelled => 130,
            Self::Spike(_) | Self::Analysis(_) | Self::Json(_) | Self::Io(_) => 10,
        }
    }
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("参数错误：{0}")]
    InvalidArgument(String),

    #[error("仓库读取失败：{0}")]
    Repository(String),

    #[error("索引不可用：{0}")]
    IndexUnavailable(String),

    #[error("索引 schema 不兼容：{0}")]
    SchemaIncompatible(String),

    #[error("DuckDB 读写失败：{0}")]
    DuckDb(String),

    #[error("gix 能力验证失败：{0}")]
    Spike(String),

    #[error("JSON 输出失败：{0}")]
    Json(#[from] serde_json::Error),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidArgument(_) => 2,
            Self::Repository(_) => 3,
            Self::IndexUnavailable(_) => 4,
            Self::SchemaIncompatible(_) => 5,
            Self::DuckDb(_) => 6,
            Self::Spike(_) | Self::Json(_) => 10,
        }
    }
}

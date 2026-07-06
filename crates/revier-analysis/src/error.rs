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

    #[error("DuckDB 读写失败：{0}")]
    DuckDb(String),

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

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidArgument(_) => 2,
            Self::Repository(_) => 3,
            Self::FileNotAnalyzable(_) | Self::IndexUnavailable(_) => 4,
            Self::RequiredIndexUnavailable(_) | Self::SchemaIncompatible(_) => 5,
            Self::DuckDb(_) => 6,
            Self::Cancelled => 130,
            Self::Spike(_) | Self::Analysis(_) | Self::Json(_) | Self::Io(_) => 10,
        }
    }
}

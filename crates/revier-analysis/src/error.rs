use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("仓库读取失败：{0}")]
    Repository(String),

    #[error("gix 能力验证失败：{0}")]
    Spike(String),

    #[error("JSON 输出失败：{0}")]
    Json(#[from] serde_json::Error),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Repository(_) => 3,
            Self::Spike(_) => 10,
            Self::Json(_) => 10,
        }
    }
}

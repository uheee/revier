use crate::cli::FileOverlayArgs;
use crate::error::AppError;

pub fn run(_args: FileOverlayArgs) -> Result<String, AppError> {
    Err(AppError::Analysis(
        "file-overlay 命令契约已注册，overlay 构建尚未接入".to_string(),
    ))
}

use crate::cli::TraceBlockArgs;
use crate::error::AppError;

pub fn run(_args: TraceBlockArgs) -> Result<String, AppError> {
    Err(AppError::Analysis(
        "trace-block 命令契约已注册，单块追踪尚未接入".to_string(),
    ))
}

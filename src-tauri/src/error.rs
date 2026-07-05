use revier_analysis::contracts::AppError;

pub type CommandResult<T> = Result<T, AppError>;

pub fn command_error(code: impl Into<String>, message: impl Into<String>) -> AppError {
    AppError {
        code: code.into(),
        message: message.into(),
        detail: None,
    }
}

#[allow(dead_code)]
pub fn command_error_with_detail(
    code: impl Into<String>,
    message: impl Into<String>,
    detail: impl Into<String>,
) -> AppError {
    AppError {
        code: code.into(),
        message: message.into(),
        detail: Some(detail.into()),
    }
}

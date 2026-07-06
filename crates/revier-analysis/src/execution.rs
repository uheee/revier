use std::sync::Arc;

use crate::error::AppError;

#[derive(Clone, Default)]
pub struct AnalysisExecutionContext {
    cancel: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
}

impl AnalysisExecutionContext {
    pub fn none() -> Self {
        Self { cancel: None }
    }

    pub fn with_cancel(cancel: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self {
            cancel: Some(Arc::new(cancel)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel
            .as_ref()
            .is_some_and(|is_cancelled| is_cancelled())
    }

    pub fn check_cancelled(&self) -> Result<(), AppError> {
        if self.is_cancelled() {
            Err(AppError::Cancelled)
        } else {
            Ok(())
        }
    }
}

use std::sync::Arc;

use crate::contracts::OperationStage;
use crate::error::AppError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperationProgressUpdate {
    pub stage: OperationStage,
    pub message: String,
    pub completed_units: Option<u64>,
    pub total_units: Option<u64>,
}

pub trait OperationProgressReporter: Send + Sync {
    fn report(&self, update: OperationProgressUpdate);
}

#[derive(Clone, Default)]
pub struct AnalysisExecutionContext {
    cancel: Option<Arc<dyn Fn() -> bool + Send + Sync>>,
    progress: Option<Arc<dyn OperationProgressReporter>>,
}

impl AnalysisExecutionContext {
    pub fn none() -> Self {
        Self {
            cancel: None,
            progress: None,
        }
    }

    pub fn with_cancel(cancel: impl Fn() -> bool + Send + Sync + 'static) -> Self {
        Self {
            cancel: Some(Arc::new(cancel)),
            progress: None,
        }
    }

    pub fn with_progress(progress: Arc<dyn OperationProgressReporter>) -> Self {
        Self {
            cancel: None,
            progress: Some(progress),
        }
    }

    pub fn with_cancel_and_progress(
        cancel: impl Fn() -> bool + Send + Sync + 'static,
        progress: Arc<dyn OperationProgressReporter>,
    ) -> Self {
        Self {
            cancel: Some(Arc::new(cancel)),
            progress: Some(progress),
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

    pub fn report_progress(&self, update: OperationProgressUpdate) {
        if let Some(progress) = self.progress.as_ref() {
            progress.report(update);
        }
    }
}

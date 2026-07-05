use std::sync::Arc;

use crate::services::projects::ProjectService;
use crate::services::review::ReviewService;

pub struct AppState {
    pub projects: Arc<ProjectService>,
    pub review: Arc<ReviewService>,
}

impl AppState {
    pub fn new(projects: ProjectService, review: ReviewService) -> Self {
        Self {
            projects: Arc::new(projects),
            review: Arc::new(review),
        }
    }
}

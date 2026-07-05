use std::sync::Arc;

use crate::services::projects::ProjectService;

pub struct AppState {
    pub projects: Arc<ProjectService>,
}

impl AppState {
    pub fn new(projects: ProjectService) -> Self {
        Self {
            projects: Arc::new(projects),
        }
    }
}

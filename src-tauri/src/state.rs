use std::sync::Arc;

use crate::services::editor_settings::EditorSettingsService;
use crate::services::projects::ProjectService;
use crate::services::review::ReviewService;

pub struct AppState {
    pub editor_settings: Arc<EditorSettingsService>,
    pub projects: Arc<ProjectService>,
    pub review: Arc<ReviewService>,
}

impl AppState {
    pub fn new(
        editor_settings: EditorSettingsService,
        projects: ProjectService,
        review: ReviewService,
    ) -> Self {
        Self {
            editor_settings: Arc::new(editor_settings),
            projects: Arc::new(projects),
            review: Arc::new(review),
        }
    }
}

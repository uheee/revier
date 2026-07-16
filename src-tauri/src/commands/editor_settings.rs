use revier_analysis::contracts::EditorSettingsSnapshot;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub fn editor_settings_get(state: State<'_, AppState>) -> EditorSettingsSnapshot {
    state.editor_settings.snapshot()
}

use std::path::Path;

use specta::Types;
use specta_typescript::Typescript;

use crate::contracts::*;

pub fn export_typescript_bindings(output: &Path) -> Result<(), std::io::Error> {
    let types = Types::default()
        .register::<TextEncoding>()
        .register::<ResolvedTextEncoding>()
        .register::<EditorThemeMode>()
        .register::<EditorFontSettings>()
        .register::<LargeFileSettings>()
        .register::<EditorSyntaxColors>()
        .register::<EditorThemeColors>()
        .register::<EditorThemes>()
        .register::<EditorSettings>()
        .register::<EditorSettingsSnapshot>()
        .register::<AppError>()
        .register::<ProjectReviewFilters>()
        .register::<ProjectPreferences>()
        .register::<ReviewProject>()
        .register::<RepositoryValidation>()
        .register::<GitBranch>()
        .register::<DirectorySelection>()
        .register::<ReviewFilters>()
        .register::<AnalysisRange>()
        .register::<AnalysisTaskStatus>()
        .register::<AnalysisStage>()
        .register::<AnalysisTaskSnapshot>()
        .register::<ChangedFileStatus>()
        .register::<ChangedFile>()
        .register::<FileOverlayRequest>()
        .register::<ReviewAuthorOptionsRequest>()
        .register::<CommitOverlayRequest>()
        .register::<AuthorSummary>()
        .register::<AuthorFilterOption>()
        .register::<AttributionConfidence>()
        .register::<AttributionWarningCode>()
        .register::<AttributionWarning>()
        .register::<BlockAttributionSummary>()
        .register::<AttributionMethod>()
        .register::<RelatedCommitAttribution>()
        .register::<TouchedRange>()
        .register::<RelatedCommit>()
        .register::<WordChange>()
        .register::<SideBySideDiffRowType>()
        .register::<SideBySideDiffRow>()
        .register::<DiffBlockChangeType>()
        .register::<DiffBlock>()
        .register::<FileOverlayMode>()
        .register::<FileOverlay>();

    let bindings = Typescript::default()
        .export(&types, specta_serde::PhasesFormat)
        .expect("生成 TypeScript bindings 失败");

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, bindings)?;
    Ok(())
}

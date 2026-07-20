use serde::{Deserialize, Serialize};
use specta::Type;

pub type ProjectId = String;
pub type TaskId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum TextEncoding {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "utf-8")]
    Utf8,
    #[serde(rename = "gb18030")]
    Gb18030,
    #[serde(rename = "utf-16le")]
    Utf16Le,
    #[serde(rename = "utf-16be")]
    Utf16Be,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum ResolvedTextEncoding {
    #[serde(rename = "utf-8")]
    Utf8,
    #[serde(rename = "gb18030")]
    Gb18030,
    #[serde(rename = "utf-16le")]
    Utf16Le,
    #[serde(rename = "utf-16be")]
    Utf16Be,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum EditorThemeMode {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorFontSettings {
    pub font_families: Vec<String>,
    pub font_size: u32,
    pub line_height: u32,
    pub minimap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LargeFileSettings {
    #[specta(type = u32)]
    pub max_bytes: u64,
    #[specta(type = u32)]
    pub max_lines: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorSyntaxColors {
    pub comment: String,
    pub keyword: String,
    pub string: String,
    pub number: String,
    pub r#type: String,
    pub function: String,
    pub variable: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorThemeColors {
    pub workspace_background: String,
    pub panel_background: String,
    pub editor_background: String,
    pub border: String,
    pub foreground: String,
    pub muted: String,
    pub accent: String,
    pub selection: String,
    pub diff_removed: String,
    pub diff_removed_strong: String,
    pub diff_removed_word: String,
    pub diff_added: String,
    pub diff_added_strong: String,
    pub diff_added_word: String,
    pub syntax: EditorSyntaxColors,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorThemes {
    pub light: EditorThemeColors,
    pub dark: EditorThemeColors,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettings {
    pub version: u32,
    pub theme: EditorThemeMode,
    pub default_encoding: TextEncoding,
    pub editor: EditorFontSettings,
    pub large_file: LargeFileSettings,
    pub themes: EditorThemes,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettingsSnapshot {
    pub settings: EditorSettings,
    pub config_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub warning: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectReviewFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub start_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub end_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::vec::Vec<std::string::String>)]
    pub author_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub author_query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub message_query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::vec::Vec<std::string::String>)]
    pub glob_rules: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPreferences {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub default_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = u32)]
    pub default_days: Option<u32>,
    pub default_glob_rules: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::ProjectReviewFilters)]
    pub review_filters: Option<ProjectReviewFilters>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProject {
    pub id: ProjectId,
    pub name: String,
    pub repo_path: String,
    pub pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub last_opened_at: Option<String>,
    pub preferences: ProjectPreferences,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryValidation {
    pub valid: bool,
    pub repo_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub current_branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DirectorySelection {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFilters {
    pub project_id: ProjectId,
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub start_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub end_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::vec::Vec<std::string::String>)]
    pub author_keys: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub author_query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub message_query: Option<String>,
    pub glob_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRange {
    pub branch: String,
    pub base_commit: String,
    pub head_commit: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub start_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub end_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum AnalysisTaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisStage {
    ReadRepository,
    ResolveRange,
    LoadCommits,
    LoadChangedFiles,
    Ready,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisTaskSnapshot {
    pub task_id: TaskId,
    pub project_id: ProjectId,
    pub status: AnalysisTaskStatus,
    pub stage: AnalysisStage,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = specta_typescript::Number)]
    pub progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::AppError)]
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum ChangedFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub old_path: Option<String>,
    pub status: ChangedFileStatus,
    #[specta(type = u32)]
    pub additions: u64,
    #[specta(type = u32)]
    pub deletions: u64,
    pub is_binary: bool,
    pub is_previewable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileOverlayRequest {
    pub task_id: TaskId,
    pub file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::TextEncoding)]
    pub encoding: Option<TextEncoding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewAuthorOptionsRequest {
    pub project_id: ProjectId,
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub start_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub end_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitOverlayRequest {
    pub task_id: TaskId,
    pub file_path: String,
    pub commit_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::TextEncoding)]
    pub encoding: Option<TextEncoding>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AuthorSummary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub email: Option<String>,
    #[specta(type = u32)]
    pub commit_count: u64,
    pub last_committed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AuthorFilterOption {
    pub key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub email: Option<String>,
    #[specta(type = u32)]
    pub commit_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum AttributionConfidence {
    Precise,
    Inferred,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttributionWarningCode {
    BlameUnavailable,
    MergeTraceAmbiguous,
    PathHistoryIncomplete,
    DeletionTraceIncomplete,
    EofNewlineAttributionUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AttributionWarning {
    pub code: AttributionWarningCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BlockAttributionSummary {
    pub confidence: AttributionConfidence,
    pub warnings: Vec<AttributionWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum AttributionMethod {
    Blame,
    MergeTrace,
    PatchInference,
    DeletionTrace,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelatedCommitAttribution {
    pub method: AttributionMethod,
    pub via_merge_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TouchedRange {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = u32)]
    pub old_start: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = u32)]
    pub old_end: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = u32)]
    pub new_start: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = u32)]
    pub new_end: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelatedCommit {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub author_email: Option<String>,
    pub committed_at: String,
    pub subject: String,
    pub matched_by_filter: bool,
    pub touched_ranges: Vec<TouchedRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::RelatedCommitAttribution)]
    pub attribution: Option<RelatedCommitAttribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WordChange {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = bool)]
    pub added: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = bool)]
    pub removed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum SideBySideDiffRowType {
    Context,
    Added,
    Deleted,
    Modified,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SideBySideDiffRow {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = u32)]
    pub old_line_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = u32)]
    pub new_line_number: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub old_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub new_text: Option<String>,
    pub r#type: SideBySideDiffRowType,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::vec::Vec<crate::contracts::WordChange>)]
    pub word_changes: Option<Vec<WordChange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub block_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum DiffBlockChangeType {
    Added,
    Deleted,
    Modified,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiffBlock {
    pub id: String,
    #[specta(type = u32)]
    pub old_start: u64,
    #[specta(type = u32)]
    pub old_end: u64,
    #[specta(type = u32)]
    pub new_start: u64,
    #[specta(type = u32)]
    pub new_end: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = u32)]
    pub row_start_index: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = u32)]
    pub row_end_index: Option<u64>,
    pub change_type: DiffBlockChangeType,
    pub authors: Vec<AuthorSummary>,
    pub rows: Vec<SideBySideDiffRow>,
    pub related_commits: Vec<RelatedCommit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::BlockAttributionSummary)]
    pub attribution: Option<BlockAttributionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiffBlockRange {
    pub id: String,
    #[specta(type = u32)]
    pub old_start: u64,
    #[specta(type = u32)]
    pub old_end: u64,
    #[specta(type = u32)]
    pub new_start: u64,
    #[specta(type = u32)]
    pub new_end: u64,
    pub change_type: DiffBlockChangeType,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AttributeBlocksRequest {
    pub task_id: TaskId,
    pub file_path: String,
    pub resolved_encoding: ResolvedTextEncoding,
    pub blocks: Vec<DiffBlockRange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiffBlockAttribution {
    pub id: String,
    pub authors: Vec<AuthorSummary>,
    pub related_commits: Vec<RelatedCommit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::BlockAttributionSummary)]
    pub attribution: Option<BlockAttributionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AttributeBlocksResult {
    pub resolved_encoding: ResolvedTextEncoding,
    pub attributions: Vec<DiffBlockAttribution>,
    pub warnings: Vec<AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum FileOverlayMode {
    Range,
    Commit,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileOverlay {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::FileOverlayMode)]
    pub mode: Option<FileOverlayMode>,
    pub file: ChangedFile,
    pub range: AnalysisRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::vec::Vec<crate::contracts::SideBySideDiffRow>)]
    pub rows: Option<Vec<SideBySideDiffRow>>,
    pub blocks: Vec<DiffBlock>,
    pub warnings: Vec<AppError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::RelatedCommit)]
    pub commit: Option<RelatedCommit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub parent_hash: Option<String>,
    pub old_content: String,
    pub new_content: String,
    pub resolved_encoding: ResolvedTextEncoding,
}

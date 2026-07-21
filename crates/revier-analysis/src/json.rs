use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpikeDecision {
    GixOnly,
    AlgorithmAdjustmentRequired,
    FallbackRisk,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Pass,
    Fail,
}

#[derive(Debug, Serialize)]
pub struct SpikeDifference {
    pub field: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Serialize)]
pub struct SpikeCheck {
    pub name: String,
    pub status: CheckStatus,
    pub differences: Vec<SpikeDifference>,
}

#[derive(Debug, Serialize)]
pub struct SpikeOutput {
    pub version: u8,
    pub fixture: String,
    pub checks: Vec<SpikeCheck>,
    pub decision: SpikeDecision,
}

impl SpikeOutput {
    pub fn new(fixture: impl Into<String>) -> Self {
        Self {
            version: 1,
            fixture: fixture.into(),
            checks: Vec::new(),
            decision: SpikeDecision::AlgorithmAdjustmentRequired,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndexStatusKind {
    Missing,
    Building,
    Ready,
    Stale,
    Incompatible,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatusOutput {
    pub version: u8,
    pub repo_id: String,
    pub schema_version: u32,
    pub status: IndexStatusKind,
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
    pub updated_at: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndexRunStatus {
    Completed,
    Failed,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexBuildOutput {
    pub version: u8,
    pub repo_id: String,
    pub status: IndexRunStatus,
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
    pub elapsed_ms: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryFilesRangeOutput {
    pub base_commit: String,
    pub head_commit: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFileOutput {
    pub path: String,
    pub old_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_blob_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_blob_id: Option<String>,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub is_binary: bool,
    pub is_previewable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryFilesOutput {
    pub version: u8,
    pub range: QueryFilesRangeOutput,
    pub files: Vec<ChangedFileOutput>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRangeOutput {
    pub branch: String,
    pub base_commit: String,
    pub head_commit: String,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthorOutput {
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AttributionWarningOutput {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockAttributionOutput {
    pub confidence: String,
    pub warnings: Vec<AttributionWarningOutput>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelatedCommitAttributionOutput {
    pub method: String,
    pub via_merge_hashes: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TouchedRangeOutput {
    pub old_start: Option<usize>,
    pub old_end: Option<usize>,
    pub new_start: Option<usize>,
    pub new_end: Option<usize>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelatedCommitOutput {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    pub author_email: Option<String>,
    pub committed_at: String,
    pub subject: String,
    pub matched_by_filter: bool,
    pub touched_ranges: Vec<TouchedRangeOutput>,
    pub attribution: Option<RelatedCommitAttributionOutput>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WordChangeOutput {
    pub value: String,
    pub added: Option<bool>,
    pub removed: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SideBySideDiffRowOutput {
    pub old_line_number: Option<usize>,
    pub new_line_number: Option<usize>,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub r#type: String,
    pub word_changes: Option<Vec<WordChangeOutput>>,
    pub block_id: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiffBlockOutput {
    pub id: String,
    pub old_start: usize,
    pub old_end: usize,
    pub new_start: usize,
    pub new_end: usize,
    pub row_start_index: Option<usize>,
    pub row_end_index: Option<usize>,
    pub change_type: String,
    pub authors: Vec<AuthorOutput>,
    pub rows: Vec<SideBySideDiffRowOutput>,
    pub related_commits: Vec<RelatedCommitOutput>,
    pub attribution: Option<BlockAttributionOutput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOverlayOutput {
    pub mode: String,
    pub file: ChangedFileOutput,
    pub range: AnalysisRangeOutput,
    pub old_content: String,
    pub new_content: String,
    pub resolved_encoding: String,
    pub rows: Vec<SideBySideDiffRowOutput>,
    pub blocks: Vec<DiffBlockOutput>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOverlayCommandOutput {
    pub version: u8,
    pub overlay: FileOverlayOutput,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceBlockOutput {
    pub version: u8,
    pub file: String,
    pub block_id: String,
    pub attribution: Option<BlockAttributionOutput>,
    pub authors: Vec<AuthorOutput>,
    pub related_commits: Vec<RelatedCommitOutput>,
    pub warnings: Vec<String>,
}

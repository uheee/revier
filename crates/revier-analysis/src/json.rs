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
pub struct QueryFilesRangeOutput {
    pub base_commit: String,
    pub head_commit: String,
}

#[derive(Debug, Serialize)]
pub struct ChangedFileOutput {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub is_binary: bool,
    pub is_previewable: bool,
}

#[derive(Debug, Serialize)]
pub struct QueryFilesOutput {
    pub version: u8,
    pub range: QueryFilesRangeOutput,
    pub files: Vec<ChangedFileOutput>,
    pub warnings: Vec<String>,
}

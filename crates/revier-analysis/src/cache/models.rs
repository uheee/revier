#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedAnalysisFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: i16,
    pub additions: u64,
    pub deletions: u64,
    pub is_binary: bool,
    pub is_previewable: bool,
    pub old_blob_id: Option<String>,
    pub new_blob_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedAnalysisSnapshot {
    pub analysis_id: String,
    pub repo_id: String,
    pub branch: String,
    pub base_commit: String,
    pub head_commit: String,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub author_query: Option<String>,
    pub message_query: Option<String>,
    pub filter_fingerprint: String,
    pub analysis_version: u32,
    pub started_at: String,
    pub completed_at: String,
    pub elapsed_ms: u64,
    pub last_selected_path: Option<String>,
    pub author_keys: Vec<String>,
    pub globs: Vec<String>,
    pub files: Vec<CachedAnalysisFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedTouchedRange {
    pub old_start: Option<u64>,
    pub old_end: Option<u64>,
    pub new_start: Option<u64>,
    pub new_end: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedBlockCommit {
    pub commit_hash: String,
    pub matched_by_filter: bool,
    pub attribution_method: Option<i16>,
    pub touched_ranges: Vec<CachedTouchedRange>,
    pub merge_hashes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedFileBlock {
    pub ordinal: u32,
    pub old_start: u64,
    pub old_end: u64,
    pub new_start: u64,
    pub new_end: u64,
    pub change_type: i16,
    pub confidence: Option<i16>,
    pub warning_flags: u32,
    pub commits: Vec<CachedBlockCommit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedFileAnalysis {
    pub file_analysis_id: String,
    pub analysis_id: String,
    pub path: String,
    pub resolved_encoding: i16,
    pub block_signature: String,
    pub analysis_version: u32,
    pub content_elapsed_ms: u64,
    pub attribution_elapsed_ms: u64,
    pub completed_at: String,
    pub blocks: Vec<CachedFileBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedCommitOverlayBlock {
    pub ordinal: u32,
    pub old_start: u64,
    pub old_end: u64,
    pub new_start: u64,
    pub new_end: u64,
    pub change_type: i16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedCommitOverlay {
    pub commit_overlay_id: String,
    pub file_analysis_id: String,
    pub commit_hash: String,
    pub parent_hash: String,
    pub historical_path: String,
    pub old_blob_id: Option<String>,
    pub new_blob_id: Option<String>,
    pub resolved_encoding: i16,
    pub analysis_version: u32,
    pub elapsed_ms: u64,
    pub completed_at: String,
    pub blocks: Vec<CachedCommitOverlayBlock>,
}

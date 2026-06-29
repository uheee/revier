use crate::cli::SpikeFixture;
use crate::error::AppError;
use crate::git::{blame, repository};
use crate::json::{CheckStatus, SpikeCheck, SpikeDecision, SpikeOutput};
use std::path::Path;

pub fn run(repo_path: &Path, fixture: &SpikeFixture) -> Result<SpikeOutput, AppError> {
    let fixture_name = fixture_name(fixture);
    let repo = repository::open_repository(repo_path)?;
    let head = repo
        .head_id()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .to_string();

    let mut output = SpikeOutput::new(fixture_name);
    let parents = repository::parent_ids(&repo, &head)?;
    output.checks.push(SpikeCheck {
        name: "gix-parent-ids".to_string(),
        status: if parents.is_empty() {
            CheckStatus::Fail
        } else {
            CheckStatus::Pass
        },
        differences: Vec::new(),
    });

    let blame_lines = blame::blame_range(&repo, &head, "src/app.txt", 1, 1).unwrap_or_default();
    output.checks.push(SpikeCheck {
        name: "gix-blame".to_string(),
        status: if blame_lines.is_empty() {
            CheckStatus::Fail
        } else {
            CheckStatus::Pass
        },
        differences: Vec::new(),
    });

    output.decision = if output
        .checks
        .iter()
        .all(|check| matches!(&check.status, CheckStatus::Pass))
    {
        SpikeDecision::GixOnly
    } else {
        SpikeDecision::AlgorithmAdjustmentRequired
    };

    Ok(output)
}

fn fixture_name(fixture: &SpikeFixture) -> &'static str {
    match fixture {
        SpikeFixture::Linear => "linear",
        SpikeFixture::EarlyFeatureMerge => "early-feature-merge",
        SpikeFixture::MergeConflict => "merge-conflict",
        SpikeFixture::MultiParentAmbiguous => "multi-parent-ambiguous",
        SpikeFixture::RenameMerge => "rename-merge",
        SpikeFixture::DeletionMerge => "deletion-merge",
    }
}

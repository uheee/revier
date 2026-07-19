use crate::cli::FileOverlayArgs;
use crate::error::AppError;
use crate::git::blob::read_blob_at_commit;
use crate::git::diff::CommitFileChange;
use crate::json::{
    AnalysisRangeOutput, ChangedFileOutput, FileOverlayCommandOutput, FileOverlayOutput,
};
use crate::overlay::diff_builder::build_overlay_diff;

pub fn build_file_overlay(
    repo: &gix::Repository,
    args: FileOverlayArgs,
) -> Result<FileOverlayCommandOutput, AppError> {
    let context = crate::attribution::context::AttributionContext::open(repo, &args.common)?;
    let mut warnings = context.warnings.clone();
    let requested_encoding = crate::text_encoding::parse_text_encoding(&args.encoding)?;

    let change = crate::git::diff::changed_file_between(
        repo,
        &args.common.base,
        &args.common.head,
        &args.file,
    )?
    .ok_or_else(|| AppError::FileNotAnalyzable(format!("文件在范围内未变更：{}", args.file)))?;

    let path_candidates = crate::attribution::path_history::path_candidates(
        &context,
        &args.common.base,
        &args.common.head,
        &change.path,
        change.old_path.as_deref(),
    )?;
    append_warnings(&mut warnings, path_candidates.warnings);

    let old_path = old_text_path(&change);
    let new_path = new_text_path(&change);
    let old_bytes = match old_path {
        Some(path) => read_blob_at_commit(repo, &args.common.base, path)?.unwrap_or_default(),
        None => Vec::new(),
    };
    let new_bytes = match new_path {
        Some(path) => read_blob_at_commit(repo, &args.common.head, path)?.unwrap_or_default(),
        None => Vec::new(),
    };
    let preferred_bytes = if new_path.is_some() {
        &new_bytes
    } else {
        &old_bytes
    };
    let resolved =
        crate::text_encoding::decode_text_bytes(preferred_bytes, requested_encoding)?.encoding;
    let concrete_encoding = crate::text_encoding::resolved_as_requested(resolved);
    let old_text = crate::text_encoding::decode_text_bytes(&old_bytes, concrete_encoding)?.text;
    let new_text = crate::text_encoding::decode_text_bytes(&new_bytes, concrete_encoding)?.text;
    if change.is_binary
        && !matches!(
            resolved,
            crate::contracts::ResolvedTextEncoding::Utf16Le
                | crate::contracts::ResolvedTextEncoding::Utf16Be
        )
    {
        return Err(AppError::FileNotAnalyzable(format!(
            "文件包含明显二进制内容：{}",
            args.file
        )));
    }
    let diff = build_overlay_diff(&old_text, &new_text);
    let attribution_options = crate::attribution::patch_inference::AttributionOptions {
        encoding: resolved,
        authors: &args.common.authors,
        author_query: args.common.author_query.as_deref(),
        message: args.common.message.as_deref(),
    };
    let blocks = crate::attribution::patch_inference::attach_patch_inference(
        &context,
        diff.blocks,
        &change.path,
        change.old_path.as_deref(),
        &attribution_options,
    )?;
    let blocks = crate::attribution::blame::attach_blame_attribution(
        &context,
        &args.common.head,
        &change.path,
        blocks,
        &attribution_options,
    )?;
    let blocks = crate::attribution::deletion_trace::attach_deletion_trace(
        &context,
        blocks,
        &path_candidates.paths,
        &change.path,
        change.old_path.as_deref(),
        &attribution_options,
    )?;

    Ok(FileOverlayCommandOutput {
        version: 1,
        overlay: FileOverlayOutput {
            mode: "range".to_string(),
            file: changed_file_output(&change),
            range: AnalysisRangeOutput {
                branch: args.common.branch,
                base_commit: args.common.base,
                head_commit: args.common.head,
                start_at: None,
                end_at: None,
            },
            old_content: old_text,
            new_content: new_text,
            resolved_encoding: crate::text_encoding::resolved_as_str(resolved).to_string(),
            rows: diff.rows,
            blocks,
            warnings: warnings.clone(),
        },
        warnings,
    })
}

fn append_warnings(warnings: &mut Vec<String>, new_warnings: Vec<String>) {
    for warning in new_warnings {
        if !warnings.contains(&warning) {
            warnings.push(warning);
        }
    }
}

fn old_text_path(change: &CommitFileChange) -> Option<&str> {
    match change.status.as_str() {
        "added" => None,
        "deleted" | "modified" => Some(change.path.as_str()),
        "renamed" => Some(change.old_path.as_deref().unwrap_or(change.path.as_str())),
        _ => Some(change.old_path.as_deref().unwrap_or(change.path.as_str())),
    }
}

fn new_text_path(change: &CommitFileChange) -> Option<&str> {
    match change.status.as_str() {
        "deleted" => None,
        "added" | "modified" | "renamed" => Some(change.path.as_str()),
        _ => Some(change.path.as_str()),
    }
}

fn changed_file_output(change: &CommitFileChange) -> ChangedFileOutput {
    ChangedFileOutput {
        path: change.path.clone(),
        old_path: change.old_path.clone(),
        status: change.status.clone(),
        additions: change.additions,
        deletions: change.deletions,
        is_binary: false,
        is_previewable: true,
    }
}

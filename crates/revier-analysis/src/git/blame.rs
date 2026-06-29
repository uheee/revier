use crate::error::AppError;
use crate::git::diff;

#[derive(Debug)]
pub struct BlameLine {
    pub line_number: usize,
    pub commit_hash: String,
}

pub fn blame_range(
    repo: &gix::Repository,
    head_commit: &str,
    path: &str,
    start_line: usize,
    end_line: usize,
) -> Result<Vec<BlameLine>, AppError> {
    if start_line == 0 || end_line < start_line {
        return Ok(Vec::new());
    }

    let suspect = repo
        .rev_parse_single(head_commit)
        .map_err(|error| AppError::Repository(error.to_string()))?
        .detach();
    let start = u32::try_from(start_line)
        .map_err(|_| AppError::Spike(format!("行号超出支持范围：{start_line}")))?;
    let end = u32::try_from(end_line)
        .map_err(|_| AppError::Spike(format!("行号超出支持范围：{end_line}")))?;
    let ranges = gix_blame::BlameRanges::from_one_based_inclusive_range(start..=end)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let mut resource_cache = diff::diff_resource_cache()?;
    let file_path = gix_object::bstr::BString::from(path.as_bytes().to_vec());
    let options = gix_blame::Options {
        ranges,
        rewrites: Some(gix_diff::Rewrites::default()),
        debug_track_path: false,
        ..Default::default()
    };

    let outcome = gix_blame::file(
        &repo.objects,
        suspect,
        None,
        &mut resource_cache,
        file_path.as_ref(),
        options,
    )
    .map_err(|error| AppError::Repository(error.to_string()))?;
    let mut lines = Vec::new();

    for entry in outcome.entries {
        for line_index in entry.range_in_blamed_file() {
            let line_number = line_index + 1;
            if (start_line..=end_line).contains(&line_number) {
                lines.push(BlameLine {
                    line_number,
                    commit_hash: entry.commit_id.to_string(),
                });
            }
        }
    }

    lines.sort_by_key(|line| line.line_number);
    Ok(lines)
}

use crate::cli::{FileOverlayArgs, TraceBlockArgs};
use crate::error::AppError;
use crate::json::{DiffBlockOutput, TraceBlockOutput};

pub fn run(args: TraceBlockArgs) -> Result<String, AppError> {
    let pretty = args.common.pretty;
    let selector = BlockSelector::from_args(&args)?;
    let repo = crate::git::repository::open_repository(&args.common.repo)?;
    let output = crate::overlay::file_overlay::build_file_overlay(
        &repo,
        FileOverlayArgs {
            common: args.common,
            file: args.file,
        },
    )?;
    let block = select_block(&output.overlay.blocks, &selector)?;

    let trace = TraceBlockOutput {
        version: 1,
        file: output.overlay.file.path,
        block_id: block.id.clone(),
        attribution: block.attribution.clone(),
        authors: block.authors.clone(),
        related_commits: block.related_commits.clone(),
        warnings: output.overlay.warnings,
    };

    crate::serialize_json(&trace, pretty)
}

#[derive(Debug)]
enum BlockSelector {
    Id(String),
    Range {
        old: Option<LineRange>,
        new: Option<LineRange>,
    },
}

#[derive(Clone, Copy, Debug)]
struct LineRange {
    start: usize,
    end: usize,
}

impl BlockSelector {
    fn from_args(args: &TraceBlockArgs) -> Result<Self, AppError> {
        if let Some(block_id) = &args.block_id {
            return Ok(Self::Id(block_id.clone()));
        }

        let old = parse_line_range(args.old_start, args.old_end, "旧侧")?;
        let new = parse_line_range(args.new_start, args.new_end, "新侧")?;
        if old.is_none() && new.is_none() {
            return Err(AppError::InvalidArgument(
                "必须提供 --block-id，或提供完整旧侧/新侧区间定位 block".to_string(),
            ));
        }

        Ok(Self::Range { old, new })
    }
}

fn parse_line_range(
    start: Option<usize>,
    end: Option<usize>,
    side_name: &str,
) -> Result<Option<LineRange>, AppError> {
    match (start, end) {
        (Some(start), Some(end)) => {
            if start == 0 || end == 0 {
                return Err(AppError::InvalidArgument(format!(
                    "{side_name}区间行号必须大于 0"
                )));
            }

            Ok(Some(LineRange { start, end }))
        }
        (None, None) => Ok(None),
        _ => Err(AppError::InvalidArgument(format!(
            "{side_name}区间必须同时提供 start 和 end"
        ))),
    }
}

fn select_block<'a>(
    blocks: &'a [DiffBlockOutput],
    selector: &BlockSelector,
) -> Result<&'a DiffBlockOutput, AppError> {
    match selector {
        BlockSelector::Id(block_id) => blocks
            .iter()
            .find(|block| block.id == *block_id)
            .ok_or_else(|| AppError::InvalidArgument(format!("找不到 block：{block_id}"))),
        BlockSelector::Range { old, new } => blocks
            .iter()
            .find(|block| block_matches_ranges(block, old, new))
            .ok_or_else(|| AppError::InvalidArgument("找不到匹配区间的 block".to_string())),
    }
}

fn block_matches_ranges(
    block: &DiffBlockOutput,
    old: &Option<LineRange>,
    new: &Option<LineRange>,
) -> bool {
    let old_matches = old
        .map(|range| block.old_start == range.start && block.old_end == range.end)
        .unwrap_or(true);
    let new_matches = new
        .map(|range| block.new_start == range.start && block.new_end == range.end)
        .unwrap_or(true);

    old_matches && new_matches
}

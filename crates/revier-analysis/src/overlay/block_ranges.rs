use std::collections::HashSet;

use crate::contracts::{DiffBlockChangeType, DiffBlockRange};
use crate::error::AppError;
use crate::json::{DiffBlockOutput, SideBySideDiffRowOutput};

pub fn split_monaco_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    text.split('\n').map(ToString::to_string).collect()
}

pub fn build_blocks_from_ranges(
    old_text: &str,
    new_text: &str,
    ranges: &[DiffBlockRange],
) -> Result<Vec<DiffBlockOutput>, AppError> {
    let old_lines = split_monaco_lines(old_text);
    let new_lines = split_monaco_lines(new_text);
    validate_ranges(ranges, old_lines.len(), new_lines.len())?;
    Ok(ranges
        .iter()
        .map(|range| build_block(range, &old_lines, &new_lines))
        .collect())
}

fn validate_ranges(
    ranges: &[DiffBlockRange],
    old_line_count: usize,
    new_line_count: usize,
) -> Result<(), AppError> {
    let mut ids = HashSet::new();
    let mut last_old_end = 0;
    let mut last_new_end = 0;

    for range in ranges {
        if range.id.trim().is_empty() {
            return invalid_range("Diff 块 ID 不能为空");
        }
        if !ids.insert(range.id.clone()) {
            return invalid_range(format!("Diff 块 ID 重复：{}", range.id));
        }
        validate_change_type(range)?;
        validate_side(range.old_start, range.old_end, old_line_count, "旧侧")?;
        validate_side(range.new_start, range.new_end, new_line_count, "新侧")?;
        if range.old_start > 0 && range.old_start <= last_old_end {
            return invalid_range(format!("Diff 块旧侧范围重叠或非单调：{}", range.id));
        }
        if range.new_start > 0 && range.new_start <= last_new_end {
            return invalid_range(format!("Diff 块新侧范围重叠或非单调：{}", range.id));
        }
        if range.old_end > 0 {
            last_old_end = range.old_end;
        }
        if range.new_end > 0 {
            last_new_end = range.new_end;
        }
    }
    Ok(())
}

fn validate_change_type(range: &DiffBlockRange) -> Result<(), AppError> {
    let has_old = range.old_start > 0 && range.old_end > 0;
    let has_new = range.new_start > 0 && range.new_end > 0;
    match range.change_type {
        DiffBlockChangeType::Added if !has_old && has_new => Ok(()),
        DiffBlockChangeType::Deleted if has_old && !has_new => Ok(()),
        DiffBlockChangeType::Modified if has_old && has_new => Ok(()),
        _ => invalid_range(format!("Diff 块类型与空侧不一致：{}", range.id)),
    }
}

fn validate_side(start: u64, end: u64, line_count: usize, label: &str) -> Result<(), AppError> {
    if start == 0 || end == 0 {
        if start == 0 && end == 0 {
            return Ok(());
        }
        return invalid_range(format!("{label}空范围必须使用 0..0"));
    }
    if end < start {
        return invalid_range(format!("{label}范围起止倒置：{start}..{end}"));
    }
    if end as usize > line_count {
        return invalid_range(format!("{label}范围越界：{start}..{end} / {line_count}"));
    }
    Ok(())
}

fn build_block(
    range: &DiffBlockRange,
    old_lines: &[String],
    new_lines: &[String],
) -> DiffBlockOutput {
    let old_count = side_len(range.old_start, range.old_end);
    let new_count = side_len(range.new_start, range.new_end);
    let row_count = old_count.max(new_count);
    let rows = (0..row_count)
        .map(|index| {
            let old_line_number = line_number_at(range.old_start, old_count, index);
            let new_line_number = line_number_at(range.new_start, new_count, index);
            SideBySideDiffRowOutput {
                old_line_number,
                new_line_number,
                old_text: old_line_number.and_then(|line| old_lines.get(line - 1).cloned()),
                new_text: new_line_number.and_then(|line| new_lines.get(line - 1).cloned()),
                r#type: row_type(range.change_type).to_string(),
                word_changes: None,
                block_id: Some(range.id.clone()),
            }
        })
        .collect();

    DiffBlockOutput {
        id: range.id.clone(),
        old_start: range.old_start as usize,
        old_end: range.old_end as usize,
        new_start: range.new_start as usize,
        new_end: range.new_end as usize,
        row_start_index: None,
        row_end_index: None,
        change_type: row_type(range.change_type).to_string(),
        authors: Vec::new(),
        rows,
        related_commits: Vec::new(),
        attribution: None,
    }
}

fn side_len(start: u64, end: u64) -> usize {
    if start == 0 || end == 0 {
        0
    } else {
        (end - start + 1) as usize
    }
}

fn line_number_at(start: u64, count: usize, index: usize) -> Option<usize> {
    (index < count).then_some(start as usize + index)
}

fn row_type(change_type: DiffBlockChangeType) -> &'static str {
    match change_type {
        DiffBlockChangeType::Added => "added",
        DiffBlockChangeType::Deleted => "deleted",
        DiffBlockChangeType::Modified => "modified",
    }
}

fn invalid_range<T>(message: impl Into<String>) -> Result<T, AppError> {
    Err(AppError::InvalidArgument(message.into()))
}

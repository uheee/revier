use crate::json::{DiffBlockOutput, SideBySideDiffRowOutput};

pub fn build_blocks(rows: &mut [SideBySideDiffRowOutput]) -> Vec<DiffBlockOutput> {
    let mut blocks = Vec::new();
    let mut current: Vec<usize> = Vec::new();

    for index in 0..rows.len() {
        if rows[index].r#type == "context" {
            flush_block(rows, &mut current, &mut blocks);
        } else {
            current.push(index);
        }
    }
    flush_block(rows, &mut current, &mut blocks);
    blocks
}

fn flush_block(
    rows: &mut [SideBySideDiffRowOutput],
    current: &mut Vec<usize>,
    blocks: &mut Vec<DiffBlockOutput>,
) {
    if current.is_empty() {
        return;
    }

    let block_id = format!("block-{}", blocks.len() + 1);
    for index in current.iter().copied() {
        rows[index].block_id = Some(block_id.clone());
    }
    let block_rows = current
        .iter()
        .map(|index| rows[*index].clone())
        .collect::<Vec<_>>();
    let old_numbers = block_rows
        .iter()
        .filter_map(|row| row.old_line_number)
        .collect::<Vec<_>>();
    let new_numbers = block_rows
        .iter()
        .filter_map(|row| row.new_line_number)
        .collect::<Vec<_>>();
    let change_type = if block_rows.iter().any(|row| row.r#type == "modified") {
        "modified"
    } else if block_rows.iter().any(|row| row.r#type == "added") {
        "added"
    } else {
        "deleted"
    };

    blocks.push(DiffBlockOutput {
        id: block_id,
        old_start: old_numbers.first().copied().unwrap_or(0),
        old_end: old_numbers.last().copied().unwrap_or(0),
        new_start: new_numbers.first().copied().unwrap_or(0),
        new_end: new_numbers.last().copied().unwrap_or(0),
        row_start_index: current.first().copied(),
        row_end_index: current.last().copied(),
        change_type: change_type.to_string(),
        authors: Vec::new(),
        rows: block_rows,
        related_commits: Vec::new(),
        attribution: None,
    });
    current.clear();
}

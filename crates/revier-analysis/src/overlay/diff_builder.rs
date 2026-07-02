use crate::json::{DiffBlockOutput, SideBySideDiffRowOutput};
use crate::overlay::blocks::build_blocks;
use crate::overlay::line_diff::diff_lines;
use crate::overlay::rows::build_rows;

pub struct BuiltOverlayDiff {
    pub rows: Vec<SideBySideDiffRowOutput>,
    pub blocks: Vec<DiffBlockOutput>,
}

pub fn build_overlay_diff(old_text: &str, new_text: &str) -> BuiltOverlayDiff {
    let parts = diff_lines(old_text, new_text);
    let mut rows = build_rows(&parts);
    let blocks = build_blocks(&mut rows);
    BuiltOverlayDiff { rows, blocks }
}

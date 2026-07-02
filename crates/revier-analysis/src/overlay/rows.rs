use crate::json::{SideBySideDiffRowOutput, WordChangeOutput};
use crate::overlay::line_diff::LineDiffPart;

pub fn build_rows(parts: &[LineDiffPart]) -> Vec<SideBySideDiffRowOutput> {
    let mut rows = Vec::new();
    let mut old_line = 1_usize;
    let mut new_line = 1_usize;
    let mut index = 0_usize;

    while index < parts.len() {
        match (&parts[index], parts.get(index + 1)) {
            (LineDiffPart::Removed(old_lines), Some(LineDiffPart::Added(new_lines))) => {
                let max_len = old_lines.len().max(new_lines.len());
                for offset in 0..max_len {
                    let old_text = old_lines.get(offset).cloned();
                    let new_text = new_lines.get(offset).cloned();
                    let old_line_number = old_text.as_ref().map(|_| {
                        let current = old_line;
                        old_line += 1;
                        current
                    });
                    let new_line_number = new_text.as_ref().map(|_| {
                        let current = new_line;
                        new_line += 1;
                        current
                    });
                    rows.push(SideBySideDiffRowOutput {
                        old_line_number,
                        new_line_number,
                        old_text: old_text.clone(),
                        new_text: new_text.clone(),
                        r#type: "modified".to_string(),
                        word_changes: word_changes(old_text.as_deref(), new_text.as_deref()),
                        block_id: None,
                    });
                }
                index += 2;
            }
            (LineDiffPart::Equal(lines), _) => {
                for line in lines {
                    rows.push(SideBySideDiffRowOutput {
                        old_line_number: Some(old_line),
                        new_line_number: Some(new_line),
                        old_text: Some(line.clone()),
                        new_text: Some(line.clone()),
                        r#type: "context".to_string(),
                        word_changes: None,
                        block_id: None,
                    });
                    old_line += 1;
                    new_line += 1;
                }
                index += 1;
            }
            (LineDiffPart::Added(lines), _) => {
                for line in lines {
                    rows.push(SideBySideDiffRowOutput {
                        old_line_number: None,
                        new_line_number: Some(new_line),
                        old_text: None,
                        new_text: Some(line.clone()),
                        r#type: "added".to_string(),
                        word_changes: None,
                        block_id: None,
                    });
                    new_line += 1;
                }
                index += 1;
            }
            (LineDiffPart::Removed(lines), _) => {
                for line in lines {
                    rows.push(SideBySideDiffRowOutput {
                        old_line_number: Some(old_line),
                        new_line_number: None,
                        old_text: Some(line.clone()),
                        new_text: None,
                        r#type: "deleted".to_string(),
                        word_changes: None,
                        block_id: None,
                    });
                    old_line += 1;
                }
                index += 1;
            }
        }
    }

    rows
}

fn word_changes(old_text: Option<&str>, new_text: Option<&str>) -> Option<Vec<WordChangeOutput>> {
    match (old_text, new_text) {
        (Some(old_text), Some(new_text)) if old_text != new_text => Some(vec![
            WordChangeOutput {
                value: old_text.to_string(),
                added: None,
                removed: Some(true),
            },
            WordChangeOutput {
                value: new_text.to_string(),
                added: Some(true),
                removed: None,
            },
        ]),
        _ => None,
    }
}

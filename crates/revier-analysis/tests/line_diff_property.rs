use proptest::prelude::*;
use revier_analysis::overlay::line_diff::{diff_lines, LineDiffPart};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]

    #[test]
    fn diff_parts_can_rebuild_generated_sides(old_text in generated_text(), new_text in generated_text()) {
        let parts = diff_lines(&old_text, &new_text);

        prop_assert_eq!(old_lines_from_parts(&parts), normalized_lines(&old_text));
        prop_assert_eq!(new_lines_from_parts(&parts), normalized_lines(&new_text));
        prop_assert!(adjacent_parts_have_distinct_kinds(&parts));
    }
}

fn generated_text() -> impl Strategy<Value = String> {
    (
        prop::collection::vec("[a-z0-9_]{0,8}", 0..24),
        any::<bool>(),
    )
        .prop_map(|(lines, trailing_newline)| {
            if lines.is_empty() {
                return String::new();
            }
            let mut text = lines.join("\n");
            if trailing_newline {
                text.push('\n');
            }
            text
        })
}

fn normalized_lines(value: &str) -> Vec<String> {
    if value.is_empty() {
        return Vec::new();
    }
    value
        .strip_suffix('\n')
        .unwrap_or(value)
        .split('\n')
        .map(ToString::to_string)
        .collect()
}

fn old_lines_from_parts(parts: &[LineDiffPart]) -> Vec<String> {
    parts
        .iter()
        .flat_map(|part| match part {
            LineDiffPart::Equal(lines) | LineDiffPart::Removed(lines) => lines.as_slice(),
            LineDiffPart::Added(_) => &[],
        })
        .cloned()
        .collect()
}

fn new_lines_from_parts(parts: &[LineDiffPart]) -> Vec<String> {
    parts
        .iter()
        .flat_map(|part| match part {
            LineDiffPart::Equal(lines) | LineDiffPart::Added(lines) => lines.as_slice(),
            LineDiffPart::Removed(_) => &[],
        })
        .cloned()
        .collect()
}

fn adjacent_parts_have_distinct_kinds(parts: &[LineDiffPart]) -> bool {
    parts.windows(2).all(|window| {
        !matches!(
            (&window[0], &window[1]),
            (LineDiffPart::Equal(_), LineDiffPart::Equal(_))
                | (LineDiffPart::Added(_), LineDiffPart::Added(_))
                | (LineDiffPart::Removed(_), LineDiffPart::Removed(_))
        )
    })
}

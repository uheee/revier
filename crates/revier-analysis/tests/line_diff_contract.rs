use imara_diff::{Algorithm, Diff, InternedInput};
use revier_analysis::overlay::line_diff::{diff_lines, LineDiffPart};

#[test]
fn empty_text_produces_no_parts() {
    assert_eq!(diff_lines("", ""), Vec::new());
}

#[test]
fn trailing_newline_presence_is_ignored_by_current_algorithm() {
    assert_eq!(
        diff_lines("one\ntwo\n", "one\ntwo"),
        vec![LineDiffPart::Equal(vec!["one".into(), "two".into()])]
    );
}

#[test]
fn repeated_lines_keep_current_lcs_anchor_choice() {
    assert_eq!(
        diff_lines("same\nold\nsame\n", "same\nsame\nnew\n"),
        vec![
            LineDiffPart::Equal(vec!["same".into()]),
            LineDiffPart::Removed(vec!["old".into()]),
            LineDiffPart::Equal(vec!["same".into()]),
            LineDiffPart::Added(vec!["new".into()]),
        ]
    );
}

#[test]
fn adjacent_added_and_removed_lines_are_merged_by_kind() {
    assert_eq!(
        diff_lines("a\nb\nc\n", "a\nx\ny\nc\n"),
        vec![
            LineDiffPart::Equal(vec!["a".into()]),
            LineDiffPart::Removed(vec!["b".into()]),
            LineDiffPart::Added(vec!["x".into(), "y".into()]),
            LineDiffPart::Equal(vec!["c".into()]),
        ]
    );
}

#[test]
fn disjoint_sides_are_reported_as_one_removed_and_one_added_block() {
    assert_eq!(
        diff_lines("old-1\nold-2\n", "new-1\nnew-2\nnew-3\n"),
        vec![
            LineDiffPart::Removed(vec!["old-1".into(), "old-2".into()]),
            LineDiffPart::Added(vec!["new-1".into(), "new-2".into(), "new-3".into()]),
        ]
    );
}

#[test]
fn parts_can_rebuild_both_sides_except_trailing_newline_marker() {
    let old_text = "one\ntwo\nfour\n";
    let new_text = "zero\none\nthree\nfour\n";
    let parts = diff_lines(old_text, new_text);

    assert_eq!(join_old_lines(&parts), "one\ntwo\nfour");
    assert_eq!(join_new_lines(&parts), "zero\none\nthree\nfour");
}

#[test]
fn imara_histogram_and_myers_preserve_rebuildable_line_sets_for_common_cases() {
    let cases = [
        ("空文本", "", ""),
        ("连续增删", "a\nb\nc\n", "a\nx\ny\nc\n"),
        ("重复行", "same\nold\nsame\n", "same\nsame\nnew\n"),
        (
            "两侧规模差异",
            &numbered_lines("old", 32),
            &numbered_lines("new", 128),
        ),
    ];

    for (name, old_text, new_text) in cases {
        for algorithm in [Algorithm::Histogram, Algorithm::Myers] {
            let summary = imara_summary(old_text, new_text, algorithm);
            assert_eq!(
                summary.rebuilt_old,
                normalize_current_lines(old_text),
                "{name} {algorithm:?} old side"
            );
            assert_eq!(
                summary.rebuilt_new,
                normalize_current_lines(new_text),
                "{name} {algorithm:?} new side"
            );
        }
    }
}

#[test]
fn imara_line_tokenization_keeps_trailing_newline_difference_visible() {
    let summary = imara_summary("one\ntwo\n", "one\ntwo", Algorithm::Histogram);

    assert_eq!(summary.removed_lines, 1);
    assert_eq!(summary.added_lines, 1);
}

#[derive(Debug)]
struct ImaraSummary {
    removed_lines: usize,
    added_lines: usize,
    rebuilt_old: String,
    rebuilt_new: String,
}

fn imara_summary(old_text: &str, new_text: &str, algorithm: Algorithm) -> ImaraSummary {
    let input = InternedInput::new(old_text, new_text);
    let mut diff = Diff::compute(algorithm, &input);
    diff.postprocess_lines(&input);

    let before_lines = current_lines(old_text);
    let after_lines = current_lines(new_text);
    let mut rebuilt_old = Vec::new();
    let mut rebuilt_new = Vec::new();
    let mut old_index = 0;
    let mut new_index = 0;
    let mut removed_lines = 0;
    let mut added_lines = 0;

    for hunk in diff.hunks() {
        let old_end = hunk.before.start as usize;
        let new_end = hunk.after.start as usize;
        rebuilt_old.extend_from_slice(&before_lines[old_index..old_end]);
        rebuilt_new.extend_from_slice(&after_lines[new_index..new_end]);

        let removed = &before_lines[hunk.before.start as usize..hunk.before.end as usize];
        let added = &after_lines[hunk.after.start as usize..hunk.after.end as usize];
        removed_lines += removed.len();
        added_lines += added.len();
        rebuilt_old.extend_from_slice(removed);
        rebuilt_new.extend_from_slice(added);

        old_index = hunk.before.end as usize;
        new_index = hunk.after.end as usize;
    }

    rebuilt_old.extend_from_slice(&before_lines[old_index..]);
    rebuilt_new.extend_from_slice(&after_lines[new_index..]);

    ImaraSummary {
        removed_lines,
        added_lines,
        rebuilt_old: rebuilt_old.join("\n"),
        rebuilt_new: rebuilt_new.join("\n"),
    }
}

fn join_old_lines(parts: &[LineDiffPart]) -> String {
    parts
        .iter()
        .flat_map(|part| match part {
            LineDiffPart::Equal(lines) | LineDiffPart::Removed(lines) => lines.as_slice(),
            LineDiffPart::Added(_) => &[],
        })
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

fn join_new_lines(parts: &[LineDiffPart]) -> String {
    parts
        .iter()
        .flat_map(|part| match part {
            LineDiffPart::Equal(lines) | LineDiffPart::Added(lines) => lines.as_slice(),
            LineDiffPart::Removed(_) => &[],
        })
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_current_lines(value: &str) -> String {
    current_lines(value).join("\n")
}

fn current_lines(value: &str) -> Vec<String> {
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

fn numbered_lines(prefix: &str, count: usize) -> String {
    (0..count)
        .map(|index| format!("{prefix}-{index}\n"))
        .collect::<String>()
}

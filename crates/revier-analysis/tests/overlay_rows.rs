use revier_analysis::overlay::diff_builder::build_overlay_diff;

#[test]
fn builds_side_by_side_rows_and_marks_changed_block_ids() {
    let diff = build_overlay_diff(
        "same before\nconst name = \"old\";\nsame after\n",
        "same before\nconst name = \"new\";\nsame after\n",
    );

    assert_eq!(diff.rows.len(), 3);
    assert_eq!(diff.blocks.len(), 1);
    assert_eq!(diff.rows[0].r#type, "context");
    assert_eq!(diff.rows[1].r#type, "modified");
    assert_eq!(diff.rows[1].block_id.as_deref(), Some("block-1"));
    assert_eq!(diff.rows[2].r#type, "context");
    assert_eq!(diff.blocks[0].row_start_index, Some(1));
    assert_eq!(diff.blocks[0].row_end_index, Some(1));
}

#[test]
fn keeps_empty_placeholders_for_added_lines() {
    let diff = build_overlay_diff("one\nthree\n", "one\ntwo\nthree\n");
    let added = diff
        .rows
        .iter()
        .find(|row| row.r#type == "added")
        .expect("存在新增行");

    assert_eq!(added.old_line_number, None);
    assert_eq!(added.new_line_number, Some(2));
    assert_eq!(added.new_text.as_deref(), Some("two"));
    assert_eq!(added.block_id.as_deref(), Some("block-1"));
}

#[test]
fn groups_adjacent_deleted_lines_into_one_block() {
    let diff = build_overlay_diff("one\ntwo\nthree\n", "one\n");

    assert_eq!(diff.blocks.len(), 1);
    assert_eq!(diff.blocks[0].change_type, "deleted");
    assert_eq!(diff.blocks[0].old_start, 2);
    assert_eq!(diff.blocks[0].old_end, 3);
    assert_eq!(diff.blocks[0].new_start, 0);
    assert_eq!(diff.blocks[0].new_end, 0);
}

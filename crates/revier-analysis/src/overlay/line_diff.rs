#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineDiffPart {
    Equal(Vec<String>),
    Added(Vec<String>),
    Removed(Vec<String>),
}

pub fn diff_lines(old_text: &str, new_text: &str) -> Vec<LineDiffPart> {
    let old_lines = split_lines(old_text);
    let new_lines = split_lines(new_text);
    let matches = lcs_pairs(&old_lines, &new_lines, 0, 0);
    parts_from_matches(&old_lines, &new_lines, &matches)
}

fn lcs_pairs(
    old_lines: &[String],
    new_lines: &[String],
    old_offset: usize,
    new_offset: usize,
) -> Vec<(usize, usize)> {
    if old_lines.is_empty() || new_lines.is_empty() {
        return Vec::new();
    }

    if old_lines.len() == 1 {
        if let Some(new_index) = new_lines.iter().position(|line| line == &old_lines[0]) {
            return vec![(old_offset, new_offset + new_index)];
        }
        return Vec::new();
    }

    if new_lines.len() == 1 {
        if let Some(old_index) = old_lines.iter().position(|line| line == &new_lines[0]) {
            return vec![(old_offset + old_index, new_offset)];
        }
        return Vec::new();
    }

    let old_mid = old_lines.len() / 2;
    let new_mid = {
        let left_scores = lcs_lengths(&old_lines[..old_mid], new_lines, false, false);
        let right_scores = lcs_lengths(&old_lines[old_mid..], new_lines, true, true);
        let mut new_mid = 0;
        let mut best_score = 0;

        for split in 0..=new_lines.len() {
            let score = left_scores[split] + right_scores[new_lines.len() - split];
            if score > best_score {
                best_score = score;
                new_mid = split;
            }
        }

        new_mid
    };

    let mut pairs = lcs_pairs(
        &old_lines[..old_mid],
        &new_lines[..new_mid],
        old_offset,
        new_offset,
    );
    pairs.extend(lcs_pairs(
        &old_lines[old_mid..],
        &new_lines[new_mid..],
        old_offset + old_mid,
        new_offset + new_mid,
    ));
    pairs
}

fn lcs_lengths(
    old_lines: &[String],
    new_lines: &[String],
    reverse_old: bool,
    reverse_new: bool,
) -> Vec<usize> {
    let mut previous = vec![0_usize; new_lines.len() + 1];
    let mut current = vec![0_usize; new_lines.len() + 1];

    for old_index in 0..old_lines.len() {
        current[0] = 0;
        let old_line = oriented_line(old_lines, old_index, reverse_old);
        for new_index in 0..new_lines.len() {
            let new_line = oriented_line(new_lines, new_index, reverse_new);
            current[new_index + 1] = if old_line == new_line {
                previous[new_index] + 1
            } else {
                previous[new_index + 1].max(current[new_index])
            };
        }
        std::mem::swap(&mut previous, &mut current);
    }

    previous
}

fn oriented_line(lines: &[String], index: usize, reversed: bool) -> &String {
    if reversed {
        &lines[lines.len() - 1 - index]
    } else {
        &lines[index]
    }
}

fn parts_from_matches(
    old_lines: &[String],
    new_lines: &[String],
    matches: &[(usize, usize)],
) -> Vec<LineDiffPart> {
    let mut parts = Vec::new();
    let mut old_index = 0;
    let mut new_index = 0;

    for (old_match, new_match) in matches.iter().copied() {
        if old_index < old_match {
            push_part(
                &mut parts,
                LineDiffPart::Removed(old_lines[old_index..old_match].to_vec()),
            );
        }
        if new_index < new_match {
            push_part(
                &mut parts,
                LineDiffPart::Added(new_lines[new_index..new_match].to_vec()),
            );
        }
        push_part(
            &mut parts,
            LineDiffPart::Equal(vec![old_lines[old_match].clone()]),
        );
        old_index = old_match + 1;
        new_index = new_match + 1;
    }

    if old_index < old_lines.len() {
        push_part(
            &mut parts,
            LineDiffPart::Removed(old_lines[old_index..].to_vec()),
        );
    }
    if new_index < new_lines.len() {
        push_part(&mut parts, LineDiffPart::Added(new_lines[new_index..].to_vec()));
    }

    parts
}

fn split_lines(value: &str) -> Vec<String> {
    if value.is_empty() {
        return Vec::new();
    }
    let trimmed = value.strip_suffix('\n').unwrap_or(value);
    trimmed.split('\n').map(ToString::to_string).collect()
}

fn push_part(parts: &mut Vec<LineDiffPart>, part: LineDiffPart) {
    match (parts.last_mut(), part) {
        (Some(LineDiffPart::Equal(existing)), LineDiffPart::Equal(mut next))
        | (Some(LineDiffPart::Added(existing)), LineDiffPart::Added(mut next))
        | (Some(LineDiffPart::Removed(existing)), LineDiffPart::Removed(mut next)) => {
            existing.append(&mut next);
        }
        (_, next) => parts.push(next),
    }
}

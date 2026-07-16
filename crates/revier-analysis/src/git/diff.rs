use crate::error::AppError;
use std::collections::BTreeSet;
use std::path::PathBuf;

#[derive(Debug)]
pub struct PairwiseChanges {
    pub parent_count: usize,
    pub paths: Vec<String>,
}

#[derive(Debug)]
pub struct RenameChange {
    pub old_path: String,
    pub new_path: String,
    pub similarity: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct CommitFileChange {
    pub commit_hash: String,
    pub parent_hash: String,
    pub parent_index: usize,
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub is_binary: bool,
    pub is_previewable: bool,
    pub similarity: Option<f32>,
}

pub fn pairwise_parent_changes(
    repo: &gix::Repository,
    commit_hash: &str,
) -> Result<PairwiseChanges, AppError> {
    let commit = resolve_commit(repo, commit_hash)?;
    let parent_ids = commit
        .parent_ids()
        .map(|parent_id| parent_id.detach())
        .collect::<Vec<_>>();
    let mut paths = BTreeSet::new();

    for parent_id in &parent_ids {
        let parent = repo
            .find_commit(*parent_id)
            .map_err(|error| AppError::Repository(error.to_string()))?;
        collect_changed_paths(repo, &parent, &commit, &mut paths)?;
    }

    Ok(PairwiseChanges {
        parent_count: parent_ids.len(),
        paths: paths.into_iter().collect(),
    })
}

fn resolve_commit<'repo>(
    repo: &'repo gix::Repository,
    commit_hash: &str,
) -> Result<gix::Commit<'repo>, AppError> {
    let object = repo
        .rev_parse_single(commit_hash)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    object
        .object()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .try_into_commit()
        .map_err(|_| AppError::Repository(format!("对象不是提交：{commit_hash}")))
}

fn collect_changed_paths(
    repo: &gix::Repository,
    parent: &gix::Commit<'_>,
    commit: &gix::Commit<'_>,
    paths: &mut BTreeSet<String>,
) -> Result<(), AppError> {
    let parent_tree = parent
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let commit_tree = commit
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;

    let parent_iter = gix_object::TreeRefIter::from_bytes(&parent_tree.data, parent_tree.id.kind());
    let commit_iter = gix_object::TreeRefIter::from_bytes(&commit_tree.data, commit_tree.id.kind());
    let mut state = gix_diff::tree::State::default();
    let mut recorder = gix_diff::tree::Recorder::default();

    gix_diff::tree(
        parent_iter,
        commit_iter,
        &mut state,
        repo.objects.clone(),
        &mut recorder,
    )
    .map_err(|error| AppError::Repository(error.to_string()))?;

    for record in recorder.records {
        let path = match record {
            gix_diff::tree::recorder::Change::Addition { path, .. }
            | gix_diff::tree::recorder::Change::Deletion { path, .. }
            | gix_diff::tree::recorder::Change::Modification { path, .. } => path,
        };
        paths.insert(String::from_utf8_lossy(path.as_ref()).into_owned());
    }

    Ok(())
}

pub fn rename_changes(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
) -> Result<Vec<RenameChange>, AppError> {
    let base = resolve_commit(repo, base_commit)?;
    let head = resolve_commit(repo, head_commit)?;
    let base_tree = base
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let head_tree = head
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let base_iter = gix_object::TreeRefIter::from_bytes(&base_tree.data, base_tree.id.kind());
    let head_iter = gix_object::TreeRefIter::from_bytes(&head_tree.data, head_tree.id.kind());
    let mut resource_cache = diff_resource_cache()?;
    let mut state = gix_diff::tree::State::default();
    let mut renames = Vec::new();
    let options = gix_diff::tree_with_rewrites::Options {
        location: Some(gix_diff::tree::recorder::Location::Path),
        rewrites: Some(gix_diff::Rewrites::default()),
    };

    gix_diff::tree_with_rewrites(
        base_iter,
        head_iter,
        &mut resource_cache,
        &mut state,
        &repo.objects,
        |change| {
            if let gix_diff::tree_with_rewrites::ChangeRef::Rewrite {
                source_location,
                location,
                copy: false,
                diff,
                ..
            } = change
            {
                renames.push(RenameChange {
                    old_path: String::from_utf8_lossy(source_location.as_ref()).into_owned(),
                    new_path: String::from_utf8_lossy(location.as_ref()).into_owned(),
                    similarity: Some(diff.map_or(1.0, |stats| stats.similarity)),
                });
            }

            Ok::<gix_diff::tree_with_rewrites::Action, std::io::Error>(
                std::ops::ControlFlow::Continue(()),
            )
        },
        options,
    )
    .map_err(|error| AppError::Repository(error.to_string()))?;

    Ok(renames)
}

pub fn commit_file_changes(
    repo: &gix::Repository,
    commit_hash: &str,
) -> Result<Vec<CommitFileChange>, AppError> {
    let commit = resolve_commit(repo, commit_hash)?;
    let parent_ids = commit
        .parent_ids()
        .map(|parent_id| parent_id.detach())
        .collect::<Vec<_>>();

    if parent_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut changes = Vec::new();
    for (parent_index, parent_id) in parent_ids.iter().enumerate() {
        let parent = repo
            .find_commit(*parent_id)
            .map_err(|error| AppError::Repository(error.to_string()))?;
        changes.extend(pairwise_file_changes(repo, &parent, &commit, parent_index)?);
    }
    Ok(changes)
}

pub fn changed_file_between(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
    requested_path: &str,
) -> Result<Option<CommitFileChange>, AppError> {
    let changes = range_file_changes(repo, base_commit, head_commit)?;

    Ok(changes.into_iter().find(|change| {
        change.path == requested_path || change.old_path.as_deref() == Some(requested_path)
    }))
}

pub fn range_file_changes(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
) -> Result<Vec<CommitFileChange>, AppError> {
    let base = resolve_commit(repo, base_commit)?;
    let head = resolve_commit(repo, head_commit)?;
    let mut changes = pairwise_file_changes(repo, &base, &head, 0)?;
    for change in &mut changes {
        populate_range_line_counts(repo, base_commit, head_commit, change)?;
    }
    Ok(changes)
}

fn populate_range_line_counts(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
    change: &mut CommitFileChange,
) -> Result<(), AppError> {
    if change.is_binary {
        return Ok(());
    }

    let old_text_result = if change.status == "added" {
        Ok(String::new())
    } else {
        crate::git::blob::read_text_at_commit(
            repo,
            base_commit,
            change.old_path.as_deref().unwrap_or(&change.path),
        )
    };
    let new_text_result = if change.status == "deleted" {
        Ok(String::new())
    } else {
        crate::git::blob::read_text_at_commit(repo, head_commit, &change.path)
    };
    let (old_text, new_text) = match (old_text_result, new_text_result) {
        (Ok(old_text), Ok(new_text)) => (old_text, new_text),
        (Err(AppError::FileNotAnalyzable(_)), _) | (_, Err(AppError::FileNotAnalyzable(_))) => {
            return Ok(())
        }
        (Err(error), _) | (_, Err(error)) => return Err(error),
    };

    for part in crate::overlay::line_diff::diff_lines(&old_text, &new_text) {
        match part {
            crate::overlay::line_diff::LineDiffPart::Added(lines) => {
                change.additions += lines.len() as u64;
            }
            crate::overlay::line_diff::LineDiffPart::Removed(lines) => {
                change.deletions += lines.len() as u64;
            }
            crate::overlay::line_diff::LineDiffPart::Equal(_) => {}
        }
    }
    Ok(())
}

fn pairwise_file_changes(
    repo: &gix::Repository,
    parent: &gix::Commit<'_>,
    commit: &gix::Commit<'_>,
    parent_index: usize,
) -> Result<Vec<CommitFileChange>, AppError> {
    let parent_tree = parent
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let commit_tree = commit
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let parent_iter = gix_object::TreeRefIter::from_bytes(&parent_tree.data, parent_tree.id.kind());
    let commit_iter = gix_object::TreeRefIter::from_bytes(&commit_tree.data, commit_tree.id.kind());
    let mut resource_cache = diff_resource_cache()?;
    let mut state = gix_diff::tree::State::default();
    let mut changes = Vec::new();

    gix_diff::tree_with_rewrites(
        parent_iter,
        commit_iter,
        &mut resource_cache,
        &mut state,
        &repo.objects,
        |change| {
            if let Some(file_change) = map_tree_change(repo, parent, commit, parent_index, change)?
            {
                changes.push(file_change);
            }
            Ok::<gix_diff::tree_with_rewrites::Action, AppError>(
                std::ops::ControlFlow::Continue(()),
            )
        },
        gix_diff::tree_with_rewrites::Options {
            location: Some(gix_diff::tree::recorder::Location::Path),
            rewrites: Some(gix_diff::Rewrites::default()),
        },
    )
    .map_err(|error| AppError::Repository(error.to_string()))?;

    Ok(changes)
}

fn map_tree_change(
    repo: &gix::Repository,
    parent: &gix::Commit<'_>,
    commit: &gix::Commit<'_>,
    parent_index: usize,
    change: gix_diff::tree_with_rewrites::ChangeRef<'_>,
) -> Result<Option<CommitFileChange>, AppError> {
    match change {
        gix_diff::tree_with_rewrites::ChangeRef::Addition {
            location,
            entry_mode,
            id,
            ..
        } => {
            if entry_mode.is_tree() {
                return Ok(None);
            }
            let is_binary = blob_contains_nul(repo, entry_mode, id)?;
            Ok(Some(file_change(
                commit,
                parent,
                parent_index,
                location,
                None,
                "added",
                is_binary,
                None,
            )))
        }
        gix_diff::tree_with_rewrites::ChangeRef::Deletion {
            location,
            entry_mode,
            id,
            ..
        } => {
            if entry_mode.is_tree() {
                return Ok(None);
            }
            let is_binary = blob_contains_nul(repo, entry_mode, id)?;
            Ok(Some(file_change(
                commit,
                parent,
                parent_index,
                location,
                None,
                "deleted",
                is_binary,
                None,
            )))
        }
        gix_diff::tree_with_rewrites::ChangeRef::Modification {
            location,
            previous_entry_mode,
            previous_id,
            entry_mode,
            id,
        } => {
            if previous_entry_mode.is_tree() || entry_mode.is_tree() {
                return Ok(None);
            }
            let is_binary = blob_contains_nul(repo, previous_entry_mode, previous_id)?
                || blob_contains_nul(repo, entry_mode, id)?;
            Ok(Some(file_change(
                commit,
                parent,
                parent_index,
                location,
                None,
                "modified",
                is_binary,
                None,
            )))
        }
        gix_diff::tree_with_rewrites::ChangeRef::Rewrite {
            source_location,
            location,
            copy: false,
            diff,
            source_entry_mode,
            entry_mode,
            id,
            ..
        } => {
            if source_entry_mode.is_tree() || entry_mode.is_tree() {
                return Ok(None);
            }
            let is_binary = blob_contains_nul(repo, entry_mode, id)?;
            Ok(Some(file_change(
                commit,
                parent,
                parent_index,
                location,
                Some(source_location),
                "renamed",
                is_binary,
                diff.map(|stats| stats.similarity),
            )))
        }
        _ => Ok(None),
    }
}

fn file_change(
    commit: &gix::Commit<'_>,
    parent: &gix::Commit<'_>,
    parent_index: usize,
    path: &gix::bstr::BStr,
    old_path: Option<&gix::bstr::BStr>,
    status: &str,
    is_binary: bool,
    similarity: Option<f32>,
) -> CommitFileChange {
    CommitFileChange {
        commit_hash: commit.id.to_string(),
        parent_hash: parent.id.to_string(),
        parent_index,
        path: String::from_utf8_lossy(path.as_ref()).into_owned(),
        old_path: old_path.map(|value| String::from_utf8_lossy(value.as_ref()).into_owned()),
        status: status.to_string(),
        additions: 0,
        deletions: 0,
        is_binary,
        is_previewable: !is_binary,
        similarity,
    }
}

fn blob_contains_nul(
    repo: &gix::Repository,
    entry_mode: gix_object::tree::EntryMode,
    id: gix::ObjectId,
) -> Result<bool, AppError> {
    if !entry_mode.is_blob_or_symlink() {
        return Ok(false);
    }

    let blob = repo
        .find_blob(id)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    Ok(crate::git::blob::is_binary_bytes(&blob.data))
}

pub(crate) fn diff_resource_cache() -> Result<gix_diff::blob::Platform, AppError> {
    let mut attributes_buffer = Vec::new();
    let mut collection = gix_attributes::search::MetadataCollection::default();
    let globals = gix_attributes::Search::new_globals(
        Vec::<PathBuf>::new(),
        &mut attributes_buffer,
        &mut collection,
    )
    .map_err(|error| AppError::Repository(error.to_string()))?;
    let attributes = gix_worktree::stack::state::Attributes::new(
        globals,
        None,
        gix_worktree::stack::state::attributes::Source::IdMapping,
        collection,
    );
    let attr_stack = gix_worktree::Stack::new(
        PathBuf::new(),
        gix_worktree::stack::State::AttributesStack(attributes),
        gix_glob::pattern::Case::Sensitive,
        Vec::new(),
        Vec::new(),
    );
    let filter = gix_filter::Pipeline::default();
    let pipeline = gix_diff::blob::Pipeline::new(
        gix_diff::blob::pipeline::WorktreeRoots::default(),
        filter,
        Vec::new(),
        gix_diff::blob::pipeline::Options::default(),
    );

    Ok(gix_diff::blob::Platform::new(
        gix_diff::blob::platform::Options::default(),
        pipeline,
        gix_diff::blob::pipeline::Mode::ToGit,
        attr_stack,
    ))
}

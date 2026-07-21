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
    pub old_blob_id: Option<String>,
    pub new_blob_id: Option<String>,
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

pub fn connected_paths_between(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
    path: &str,
    old_path: Option<&str>,
) -> Result<Vec<String>, AppError> {
    let mut paths = vec![path.to_string()];
    if let Some(old_path) = old_path {
        push_unique_path(&mut paths, old_path);
    }
    let renames = rename_changes(repo, base_commit, head_commit)?;
    let mut changed = true;
    while changed {
        changed = false;
        for rename in &renames {
            if paths
                .iter()
                .any(|path| path == &rename.old_path || path == &rename.new_path)
            {
                changed |= push_unique_path(&mut paths, &rename.old_path);
                changed |= push_unique_path(&mut paths, &rename.new_path);
            }
        }
    }
    Ok(paths)
}

fn push_unique_path(paths: &mut Vec<String>, path: &str) -> bool {
    if paths.iter().any(|existing| existing == path) {
        return false;
    }
    paths.push(path.to_string());
    true
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
        let empty_tree = repo.empty_tree();
        let commit_tree = commit
            .tree()
            .map_err(|error| AppError::Repository(error.to_string()))?;
        return file_changes_between_trees(
            repo,
            &empty_tree,
            &commit_tree,
            &empty_tree.id.to_string(),
            &commit,
            0,
            true,
        );
    }

    let mut changes = Vec::new();
    for (parent_index, parent_id) in parent_ids.iter().enumerate() {
        let parent = repo
            .find_commit(*parent_id)
            .map_err(|error| AppError::Repository(error.to_string()))?;
        changes.extend(pairwise_file_changes(
            repo,
            &parent,
            &commit,
            parent_index,
            true,
        )?);
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
    let mut changes = pairwise_file_changes(repo, &base, &head, 0, true)?;
    populate_range_file_statistics(repo, base_commit, head_commit, &mut changes)?;
    Ok(changes)
}

pub fn range_file_tree_changes(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
) -> Result<Vec<CommitFileChange>, AppError> {
    let base = resolve_commit(repo, base_commit)?;
    let head = resolve_commit(repo, head_commit)?;
    pairwise_file_changes(repo, &base, &head, 0, false)
}

pub fn populate_range_file_statistics(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
    changes: &mut [CommitFileChange],
) -> Result<(), AppError> {
    for change in changes {
        populate_range_line_counts(repo, base_commit, head_commit, change)?;
    }
    Ok(())
}

fn populate_range_line_counts(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
    change: &mut CommitFileChange,
) -> Result<(), AppError> {
    let old_bytes_result = if change.status == "added" {
        Ok(Vec::new())
    } else {
        crate::git::blob::read_blob_at_commit(
            repo,
            base_commit,
            change.old_path.as_deref().unwrap_or(&change.path),
        )
        .map(|bytes| bytes.unwrap_or_default())
    };
    let new_bytes_result = if change.status == "deleted" {
        Ok(Vec::new())
    } else {
        crate::git::blob::read_blob_at_commit(repo, head_commit, &change.path)
            .map(|bytes| bytes.unwrap_or_default())
    };
    let (old_bytes, new_bytes) = match (old_bytes_result, new_bytes_result) {
        (Ok(old_bytes), Ok(new_bytes)) => (old_bytes, new_bytes),
        (Err(error), _) | (_, Err(error)) => return Err(error),
    };
    change.is_binary = crate::git::blob::is_binary_bytes(&old_bytes)
        || crate::git::blob::is_binary_bytes(&new_bytes);
    change.is_previewable = !change.is_binary;
    if change.is_binary {
        return Ok(());
    }
    let old_text = match String::from_utf8(old_bytes) {
        Ok(text) => text,
        Err(_) => return Ok(()),
    };
    let new_text = match String::from_utf8(new_bytes) {
        Ok(text) => text,
        Err(_) => return Ok(()),
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
    inspect_content: bool,
) -> Result<Vec<CommitFileChange>, AppError> {
    let parent_tree = parent
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let commit_tree = commit
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    file_changes_between_trees(
        repo,
        &parent_tree,
        &commit_tree,
        &parent.id.to_string(),
        commit,
        parent_index,
        inspect_content,
    )
}

fn file_changes_between_trees(
    repo: &gix::Repository,
    parent_tree: &gix::Tree<'_>,
    commit_tree: &gix::Tree<'_>,
    parent_hash: &str,
    commit: &gix::Commit<'_>,
    parent_index: usize,
    inspect_content: bool,
) -> Result<Vec<CommitFileChange>, AppError> {
    let parent_iter = gix_object::TreeRefIter::from_bytes(&parent_tree.data, parent_tree.id.kind());
    let commit_iter = gix_object::TreeRefIter::from_bytes(&commit_tree.data, commit_tree.id.kind());
    let mut resource_cache = diff_resource_cache()?;
    let mut state = gix_diff::tree::State::default();
    let mut changes = Vec::new();
    let rewrites = if inspect_content {
        gix_diff::Rewrites::default()
    } else {
        gix_diff::Rewrites {
            percentage: None,
            ..gix_diff::Rewrites::default()
        }
    };

    gix_diff::tree_with_rewrites(
        parent_iter,
        commit_iter,
        &mut resource_cache,
        &mut state,
        &repo.objects,
        |change| {
            if let Some(file_change) = map_tree_change(
                repo,
                parent_hash,
                commit,
                parent_index,
                change,
                inspect_content,
            )? {
                changes.push(file_change);
            }
            Ok::<gix_diff::tree_with_rewrites::Action, AppError>(
                std::ops::ControlFlow::Continue(()),
            )
        },
        gix_diff::tree_with_rewrites::Options {
            location: Some(gix_diff::tree::recorder::Location::Path),
            rewrites: Some(rewrites),
        },
    )
    .map_err(|error| AppError::Repository(error.to_string()))?;

    Ok(changes)
}

fn map_tree_change(
    repo: &gix::Repository,
    parent_hash: &str,
    commit: &gix::Commit<'_>,
    parent_index: usize,
    change: gix_diff::tree_with_rewrites::ChangeRef<'_>,
    inspect_content: bool,
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
            let is_binary = inspect_content && blob_contains_nul(repo, entry_mode, id)?;
            Ok(Some(file_change(
                commit,
                parent_hash,
                parent_index,
                FileChangeDetails {
                    path: location,
                    old_path: None,
                    status: "added",
                    is_binary,
                    similarity: None,
                    old_blob_id: None,
                    new_blob_id: Some(id),
                },
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
            let is_binary = inspect_content && blob_contains_nul(repo, entry_mode, id)?;
            Ok(Some(file_change(
                commit,
                parent_hash,
                parent_index,
                FileChangeDetails {
                    path: location,
                    old_path: None,
                    status: "deleted",
                    is_binary,
                    similarity: None,
                    old_blob_id: Some(id),
                    new_blob_id: None,
                },
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
            let is_binary = inspect_content
                && (blob_contains_nul(repo, previous_entry_mode, previous_id)?
                    || blob_contains_nul(repo, entry_mode, id)?);
            Ok(Some(file_change(
                commit,
                parent_hash,
                parent_index,
                FileChangeDetails {
                    path: location,
                    old_path: None,
                    status: "modified",
                    is_binary,
                    similarity: None,
                    old_blob_id: Some(previous_id),
                    new_blob_id: Some(id),
                },
            )))
        }
        gix_diff::tree_with_rewrites::ChangeRef::Rewrite {
            source_location,
            location,
            copy: false,
            diff,
            source_entry_mode,
            source_id,
            entry_mode,
            id,
            ..
        } => {
            if source_entry_mode.is_tree() || entry_mode.is_tree() {
                return Ok(None);
            }
            let is_binary = inspect_content
                && (blob_contains_nul(repo, source_entry_mode, source_id)?
                    || blob_contains_nul(repo, entry_mode, id)?);
            Ok(Some(file_change(
                commit,
                parent_hash,
                parent_index,
                FileChangeDetails {
                    path: location,
                    old_path: Some(source_location),
                    status: "renamed",
                    is_binary,
                    similarity: diff.map(|stats| stats.similarity),
                    old_blob_id: Some(source_id),
                    new_blob_id: Some(id),
                },
            )))
        }
        _ => Ok(None),
    }
}

fn file_change(
    commit: &gix::Commit<'_>,
    parent_hash: &str,
    parent_index: usize,
    details: FileChangeDetails<'_>,
) -> CommitFileChange {
    CommitFileChange {
        commit_hash: commit.id.to_string(),
        parent_hash: parent_hash.to_string(),
        parent_index,
        path: String::from_utf8_lossy(details.path.as_ref()).into_owned(),
        old_path: details
            .old_path
            .map(|value| String::from_utf8_lossy(value.as_ref()).into_owned()),
        status: details.status.to_string(),
        additions: 0,
        deletions: 0,
        is_binary: details.is_binary,
        is_previewable: !details.is_binary,
        similarity: details.similarity,
        old_blob_id: details.old_blob_id.map(|id| id.to_string()),
        new_blob_id: details.new_blob_id.map(|id| id.to_string()),
    }
}

struct FileChangeDetails<'a> {
    path: &'a gix::bstr::BStr,
    old_path: Option<&'a gix::bstr::BStr>,
    status: &'a str,
    is_binary: bool,
    similarity: Option<f32>,
    old_blob_id: Option<gix::ObjectId>,
    new_blob_id: Option<gix::ObjectId>,
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

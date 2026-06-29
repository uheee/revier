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

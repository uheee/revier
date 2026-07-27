use crate::attribution::context::AttributionContext;
use crate::error::AppError;

#[derive(Debug)]
pub struct PathCandidates {
    pub paths: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn path_candidates(
    context: &AttributionContext<'_>,
    base: &str,
    head: &str,
    path: &str,
    old_path: Option<&str>,
) -> Result<PathCandidates, AppError> {
    let mut paths = Vec::new();
    push_unique(&mut paths, path);
    if let Some(old_path) = old_path {
        push_unique(&mut paths, old_path);
    }

    let renames = crate::git::diff::rename_changes(context.repo, base, head)?;
    let mut changed = true;
    while changed {
        changed = false;
        for rename in &renames {
            if paths
                .iter()
                .any(|path| path == &rename.old_path || path == &rename.new_path)
            {
                changed |= push_unique(&mut paths, &rename.old_path);
                changed |= push_unique(&mut paths, &rename.new_path);
            }
        }
    }

    Ok(PathCandidates {
        paths,
        warnings: Vec::new(),
    })
}

pub fn parent_path_candidates(
    context: &AttributionContext<'_>,
    parent: &str,
    child: &str,
    path: &str,
) -> Result<PathCandidates, AppError> {
    path_candidates(context, parent, child, path, None)
}

fn push_unique(paths: &mut Vec<String>, path: &str) -> bool {
    if paths.iter().any(|existing| existing == path) {
        return false;
    }
    paths.push(path.to_string());
    true
}

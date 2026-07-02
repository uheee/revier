use crate::attribution::context::AttributionContext;
use crate::error::AppError;
use crate::git::commits::IndexedCommit;

pub fn get_commit(context: &AttributionContext<'_>, hash: &str) -> Result<IndexedCommit, AppError> {
    if let Some(conn) = context.db.as_ref() {
        if let Some(commit) = crate::index::queries::commit_metadata(conn, hash)? {
            return Ok(commit);
        }
    }

    crate::git::commits::get_commit(context.repo, hash)
}

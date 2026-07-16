use crate::error::AppError;
use gix::bstr::ByteSlice;

pub fn read_blob_at_commit(
    repo: &gix::Repository,
    commit_hash: &str,
    path: &str,
) -> Result<Option<Vec<u8>>, AppError> {
    let object = repo
        .rev_parse_single(commit_hash)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let commit = object
        .object()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .try_into_commit()
        .map_err(|_| AppError::Repository(format!("对象不是提交：{commit_hash}")))?;
    let tree = commit
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let mut current = tree;
    let mut components = path.split('/').filter(|part| !part.is_empty()).peekable();

    while let Some(component) = components.next() {
        let entry = current
            .iter()
            .find_map(|entry| match entry {
                Ok(entry) if entry.inner.filename.to_str_lossy() == component => Some(Ok(entry)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .transpose()
            .map_err(|error| AppError::Repository(error.to_string()))?;
        let Some(entry) = entry else {
            return Ok(None);
        };
        let entry_mode = entry.inner.mode;
        let entry_oid = entry.inner.oid.to_owned();

        if components.peek().is_none() {
            if !entry_mode.is_blob_or_symlink() {
                return Ok(None);
            }
            let blob = repo
                .find_blob(entry_oid)
                .map_err(|error| AppError::Repository(error.to_string()))?;
            return Ok(Some(blob.data.to_vec()));
        }

        if !entry_mode.is_tree() {
            return Ok(None);
        }

        current = repo
            .find_tree(entry_oid)
            .map_err(|error| AppError::Repository(error.to_string()))?;
    }

    Ok(None)
}

pub fn read_text_at_commit(
    repo: &gix::Repository,
    commit_hash: &str,
    path: &str,
) -> Result<String, AppError> {
    let Some(bytes) = read_blob_at_commit(repo, commit_hash, path)? else {
        return Ok(String::new());
    };
    crate::text_encoding::decode_text_bytes(&bytes, crate::contracts::TextEncoding::Auto)
        .map(|decoded| decoded.text)
        .map_err(|error| match error {
            AppError::FileNotAnalyzable(message) => {
                AppError::FileNotAnalyzable(format!("{path}：{message}"))
            }
            other => other,
        })
}

pub fn is_binary_at_commit(
    repo: &gix::Repository,
    commit_hash: &str,
    path: &str,
) -> Result<bool, AppError> {
    Ok(read_blob_at_commit(repo, commit_hash, path)?
        .as_deref()
        .map(is_binary_bytes)
        .unwrap_or(false))
}

pub(crate) fn is_binary_bytes(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        return false;
    }
    bytes.iter().take(8000).any(|byte| *byte == 0)
}

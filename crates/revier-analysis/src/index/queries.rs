use crate::error::AppError;
use crate::git::commits::IndexedCommit;
use crate::json::ChangedFileOutput;
use chrono::{DateTime, Utc};
use duckdb::params;
use globset::{Glob, GlobSet, GlobSetBuilder};

pub struct IndexStatusRecord {
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
    pub updated_at: Option<String>,
}

pub fn status_record(conn: &duckdb::Connection) -> Result<IndexStatusRecord, AppError> {
    let indexed_commit_count =
        conn.query_row("select count(*) from commits", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| AppError::DuckDb(error.to_string()))? as u64;
    let indexed_file_count = conn
        .query_row("select count(*) from commit_files", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| AppError::DuckDb(error.to_string()))? as u64;
    let updated_at = conn
        .query_row(
            "select value from metadata where key = 'updated_at'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok();

    Ok(IndexStatusRecord {
        indexed_commit_count,
        indexed_file_count,
        updated_at,
    })
}

pub fn commit_exists(conn: &duckdb::Connection, hash: &str) -> Result<bool, AppError> {
    conn.query_row(
        "select count(*) > 0 from commits where hash = ?",
        params![hash],
        |row| row.get(0),
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))
}

pub fn parent_hashes(conn: &duckdb::Connection, hash: &str) -> Result<Vec<String>, AppError> {
    let mut stmt = conn
        .prepare(
            "select parent_hash
             from commit_parents
             where commit_hash = ?
             order by parent_index",
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    let rows = stmt
        .query_map(params![hash], |row| row.get::<_, String>(0))
        .map_err(|error| AppError::DuckDb(error.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::DuckDb(error.to_string()))
}

pub fn commit_metadata(
    conn: &duckdb::Connection,
    hash: &str,
) -> Result<Option<IndexedCommit>, AppError> {
    let row = conn.query_row(
        "select hash, short_hash, author_name, author_email, author_key, strftime(committed_at, '%Y-%m-%dT%H:%M:%S+00:00'), subject, is_merge
         from commits
         where hash = ?",
        params![hash],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, bool>(7)?,
            ))
        },
    );

    let (hash, short_hash, author_name, author_email, author_key, committed_at, subject, is_merge) =
        match row {
            Ok(row) => row,
            Err(duckdb::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(AppError::DuckDb(error.to_string())),
        };

    Ok(Some(IndexedCommit {
        parents: parent_hashes(conn, &hash)?,
        hash,
        short_hash,
        author_name,
        author_email,
        author_key,
        committed_at,
        subject,
        is_merge,
    }))
}

pub struct QueryFilesFilter {
    pub base: String,
    pub head: String,
    pub authors: Vec<String>,
    pub author_query: Option<String>,
    pub message: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub globs: Vec<String>,
}

pub fn query_files(
    conn: &duckdb::Connection,
    filter: &QueryFilesFilter,
) -> Result<Vec<ChangedFileOutput>, AppError> {
    ensure_range_indexed(conn, &filter.base, &filter.head)?;
    let matcher = build_glob_matcher(&filter.globs)?;
    let commits = matching_commits(conn, filter)?;
    if commits.is_empty() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for commit_hash in commits {
        let mut stmt = conn
            .prepare(
                "select path, old_path, status, additions, deletions, is_binary, is_previewable
                 from commit_files
                 where commit_hash = ?
                 order by path",
            )
            .map_err(|error| AppError::DuckDb(error.to_string()))?;
        let rows = stmt
            .query_map([commit_hash], |row| {
                let old_path: String = row.get(1)?;
                Ok(ChangedFileOutput {
                    path: row.get(0)?,
                    old_path: (!old_path.is_empty()).then_some(old_path),
                    status: row.get(2)?,
                    additions: row.get::<_, i64>(3)? as u64,
                    deletions: row.get::<_, i64>(4)? as u64,
                    is_binary: row.get(5)?,
                    is_previewable: row.get(6)?,
                })
            })
            .map_err(|error| AppError::DuckDb(error.to_string()))?;

        for file in rows {
            let file = file.map_err(|error| AppError::DuckDb(error.to_string()))?;
            if path_matches(&matcher, &file.path, file.old_path.as_deref()) {
                files.push(file);
            }
        }
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    files.dedup_by(|left, right| left.path == right.path && left.old_path == right.old_path);
    Ok(files)
}

fn ensure_range_indexed(conn: &duckdb::Connection, base: &str, head: &str) -> Result<(), AppError> {
    let indexed_head: i64 = conn
        .query_row(
            "select count(*) from commits where hash = ?",
            [head],
            |row| row.get(0),
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    if indexed_head == 0 {
        return Err(AppError::IndexUnavailable(format!(
            "查询范围 head {head} 未被索引，请先执行 index build"
        )));
    }

    let indexed_base: i64 = conn
        .query_row(
            "select count(*) from commits where hash = ?",
            [base],
            |row| row.get(0),
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    if indexed_base == 0 {
        return Err(AppError::IndexUnavailable(format!(
            "查询范围 base {base} 未被索引，请先执行 index build"
        )));
    }
    Ok(())
}

fn matching_commits(
    conn: &duckdb::Connection,
    filter: &QueryFilesFilter,
) -> Result<Vec<String>, AppError> {
    let mut stmt = conn
        .prepare(
            "select hash, author_key, author_name, coalesce(author_email, ''), subject, strftime(committed_at, '%Y-%m-%dT%H:%M:%S+00:00')
             from commits",
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|error| AppError::DuckDb(error.to_string()))?;

    let normalized_authors = filter
        .authors
        .iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let author_query = normalized_query(filter.author_query.as_deref());
    let message_query = normalized_query(filter.message.as_deref());
    let since = parse_optional_filter_time(filter.since.as_deref(), "since")?;
    let until = parse_optional_filter_time(filter.until.as_deref(), "until")?;

    let mut hashes = Vec::new();
    for row in rows {
        let (hash, author_key, author_name, author_email, subject, committed_at) =
            row.map_err(|error| AppError::DuckDb(error.to_string()))?;
        let committed_at = parse_filter_time(&committed_at, "committed_at")?;
        if !normalized_authors.is_empty() && !normalized_authors.contains(&author_key) {
            continue;
        }
        let author_text = format!("{author_name} {author_email}").to_lowercase();
        if let Some(query) = &author_query {
            if !author_text.contains(query) {
                continue;
            }
        }
        if let Some(query) = &message_query {
            if !subject.to_lowercase().contains(query) {
                continue;
            }
        }
        if let Some(since) = since {
            if committed_at < since {
                continue;
            }
        }
        if let Some(until) = until {
            if committed_at > until {
                continue;
            }
        }
        hashes.push(hash);
    }
    Ok(hashes)
}

fn normalized_query(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
}

fn parse_optional_filter_time(
    value: Option<&str>,
    field: &str,
) -> Result<Option<DateTime<Utc>>, AppError> {
    value
        .map(|value| parse_filter_time(value, field))
        .transpose()
}

fn parse_filter_time(value: &str, field: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| AppError::InvalidArgument(format!("{field} 时间格式无效：{error}")))
}

struct GlobMatcher {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

fn build_glob_matcher(rules: &[String]) -> Result<GlobMatcher, AppError> {
    let mut include = GlobSetBuilder::new();
    let mut exclude = GlobSetBuilder::new();
    let mut include_count = 0;
    let mut exclude_count = 0;
    for rule in rules {
        if let Some(stripped) = rule.strip_prefix('!') {
            exclude.add(
                Glob::new(stripped)
                    .map_err(|error| AppError::InvalidArgument(error.to_string()))?,
            );
            exclude_count += 1;
        } else {
            include.add(
                Glob::new(rule).map_err(|error| AppError::InvalidArgument(error.to_string()))?,
            );
            include_count += 1;
        }
    }
    Ok(GlobMatcher {
        include: (include_count > 0)
            .then(|| {
                include
                    .build()
                    .map_err(|error| AppError::InvalidArgument(error.to_string()))
            })
            .transpose()?,
        exclude: (exclude_count > 0)
            .then(|| {
                exclude
                    .build()
                    .map_err(|error| AppError::InvalidArgument(error.to_string()))
            })
            .transpose()?,
    })
}

fn path_matches(matcher: &GlobMatcher, path: &str, old_path: Option<&str>) -> bool {
    let candidates = old_path.map_or_else(|| vec![path], |old| vec![path, old]);
    let included = matcher.include.as_ref().map_or(true, |set| {
        candidates.iter().any(|candidate| set.is_match(candidate))
    });
    let excluded = matcher.exclude.as_ref().map_or(false, |set| {
        candidates.iter().any(|candidate| set.is_match(candidate))
    });
    included && !excluded
}

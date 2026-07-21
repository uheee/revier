use tempfile::tempdir;

#[test]
fn migrates_version_one_and_preserves_commit_index() {
    let dir = tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");
    create_version_one_schema(&conn);
    conn.execute(
        "insert into commits
         (hash, short_hash, author_name, author_email, author_key, committed_at, subject, parent_count, is_merge)
         values ('abc', 'abc', '测试作者', null, '测试作者', '2026-07-21T00:00:00Z', '测试提交', 0, false)",
        [],
    )
    .expect("写入版本一提交");

    revier_analysis::index::migrations::ensure_compatible_schema(&conn).expect("迁移 schema");

    assert_eq!(
        revier_analysis::index::schema::read_schema_version(&conn).expect("读取 schema 版本"),
        Some(3)
    );
    let commit_count: i64 = conn
        .query_row(
            "select count(*) from commits where hash = 'abc'",
            [],
            |row| row.get(0),
        )
        .expect("读取提交数量");
    assert_eq!(commit_count, 1);
    let tables = revier_analysis::index::schema::list_tables(&conn).expect("读取表列表");
    for table in [
        "analysis_snapshots",
        "analysis_files",
        "file_analyses",
        "file_blocks",
        "file_block_commits",
        "commit_overlays",
        "commit_overlay_blocks",
    ] {
        assert!(tables.contains(&table.to_string()), "缺少缓存表：{table}");
    }
}

#[test]
fn version_one_migration_is_idempotent() {
    let dir = tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");
    create_version_one_schema(&conn);

    revier_analysis::index::migrations::ensure_compatible_schema(&conn).expect("首次迁移 schema");
    revier_analysis::index::migrations::ensure_compatible_schema(&conn).expect("重复迁移 schema");

    assert_eq!(
        revier_analysis::index::schema::read_schema_version(&conn).expect("读取 schema 版本"),
        Some(3)
    );
}

#[test]
fn migrates_version_two_snapshot_and_preserves_cached_analysis() {
    let dir = tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");
    conn.execute_batch(
        "create table metadata (key text primary key, value text not null);
         insert into metadata values ('schema_version', '2');
         create table analysis_snapshots (
           analysis_id text primary key, repo_id text not null, branch text not null,
           base_commit text not null, head_commit text not null, start_at timestamp,
           end_at timestamp, author_query text, message_query text,
           filter_fingerprint text not null, analysis_version integer not null,
           started_at timestamp not null, completed_at timestamp not null,
           elapsed_ms bigint not null, unique (repo_id, branch)
         );
         insert into analysis_snapshots values (
           'analysis-1', 'repo-1', 'main', 'base', 'head', null, null, null, null,
           'fingerprint', 1, '2026-07-21T00:00:00Z', '2026-07-21T00:00:01Z', 1000
         );",
    )
    .expect("创建版本二 schema");

    revier_analysis::index::migrations::ensure_compatible_schema(&conn).expect("迁移版本二 schema");

    assert_eq!(
        revier_analysis::index::schema::read_schema_version(&conn).expect("读取 schema 版本"),
        Some(3)
    );
    let cached: (String, Option<String>) = conn
        .query_row(
            "select analysis_id, last_selected_path from analysis_snapshots",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("读取迁移后的缓存快照");
    assert_eq!(cached, ("analysis-1".to_string(), None));
}

#[test]
fn failed_migration_keeps_version_one_metadata() {
    let dir = tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");
    create_version_one_schema(&conn);
    conn.execute_batch("create table analysis_snapshots (broken text);")
        .expect("创建冲突表");

    let result = revier_analysis::index::migrations::ensure_compatible_schema(&conn);

    assert!(result.is_err());
    assert_eq!(
        revier_analysis::index::schema::read_schema_version(&conn).expect("读取 schema 版本"),
        Some(1)
    );
}

#[test]
fn cache_schema_does_not_store_source_content_columns() {
    let dir = tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");
    revier_analysis::index::schema::initialize_schema(
        &conn,
        "repo-1",
        "E:/repo/app",
        "E:/repo/app/.git",
    )
    .expect("初始化 schema");

    let content_columns: i64 = conn
        .query_row(
            "select count(*) from information_schema.columns
             where table_name in ('analysis_files', 'file_analyses', 'commit_overlays')
               and column_name in ('old_content', 'new_content')",
            [],
            |row| row.get(0),
        )
        .expect("读取缓存列");
    assert_eq!(content_columns, 0);

    let duplicated_commit_metadata_columns: i64 = conn
        .query_row(
            "select count(*) from information_schema.columns
             where table_name in ('file_block_commits', 'file_block_merge_sources')
               and column_name in ('author_name', 'author_email', 'author_key', 'committed_at', 'subject')",
            [],
            |row| row.get(0),
        )
        .expect("读取提交元数据列");
    assert_eq!(duplicated_commit_metadata_columns, 0);
}

fn create_version_one_schema(conn: &duckdb::Connection) {
    conn.execute_batch(
        "create table metadata (key text primary key, value text not null);
         insert into metadata values ('schema_version', '1');
         create table commits (
           hash text primary key,
           short_hash text not null,
           author_name text not null,
           author_email text,
           author_key text not null,
           committed_at timestamp not null,
           subject text not null,
           parent_count integer not null,
           is_merge boolean not null
         );
         create table commit_parents (
           commit_hash text not null,
           parent_hash text not null,
           parent_index integer not null,
           primary key (commit_hash, parent_index)
         );
         create table commit_files (
           commit_hash text not null,
           parent_hash text not null,
           parent_index integer not null,
           path text not null,
           old_path text not null,
           status text not null,
           additions integer not null,
           deletions integer not null,
           is_binary boolean not null,
           is_previewable boolean not null,
           similarity real,
           primary key (commit_hash, parent_index, path, old_path)
         );
         create table index_runs (
           run_id text primary key,
           repo_id text not null,
           started_at timestamp not null,
           finished_at timestamp,
           status text not null,
           indexed_commit_count integer not null,
           indexed_file_count integer not null,
           elapsed_ms integer,
           error_message text
         );",
    )
    .expect("创建版本一 schema");
}

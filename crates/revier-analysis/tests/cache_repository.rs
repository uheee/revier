use revier_analysis::cache::models::{
    CachedAnalysisFile, CachedAnalysisSnapshot, CachedBlockCommit, CachedCommitOverlay,
    CachedCommitOverlayBlock, CachedFileAnalysis, CachedFileBlock, CachedTouchedRange,
};
use revier_analysis::cache::repository::{
    branch_cache_state, cleanup_orphaned_cache_rows, delete_file_commit_overlays,
    load_branch_snapshot, load_commit_overlay, load_file_analysis, publish_branch_snapshot,
    publish_branch_snapshot_and_prune, replace_commit_overlay, replace_file_analysis,
    update_last_selected_path,
};
use revier_analysis::contracts::CacheState;
use tempfile::tempdir;

#[test]
fn publishes_and_loads_one_snapshot_per_branch() {
    let (_dir, conn) = database();
    let main = snapshot("analysis-main-1", "main", "head-main-1");
    publish_branch_snapshot(&conn, &main).expect("发布 main 快照");

    let loaded = load_branch_snapshot(&conn, "repo-1", "main")
        .expect("读取 main 快照")
        .expect("main 快照应存在");

    assert_eq!(loaded.analysis_id, main.analysis_id);
    assert_eq!(loaded.branch, "main");
    assert_eq!(loaded.head_commit, "head-main-1");
    assert_eq!(loaded.author_keys, vec!["alice@example.com"]);
    assert_eq!(loaded.globs, vec!["src/**/*.rs"]);
    assert_eq!(loaded.files, main.files);
}

#[test]
fn replaces_same_branch_and_keeps_other_branch() {
    let (_dir, conn) = database();
    publish_branch_snapshot(&conn, &snapshot("analysis-main-1", "main", "head-main-1"))
        .expect("发布旧 main 快照");
    publish_branch_snapshot(
        &conn,
        &snapshot("analysis-feature-1", "feature", "head-feature-1"),
    )
    .expect("发布 feature 快照");
    publish_branch_snapshot(&conn, &snapshot("analysis-main-2", "main", "head-main-2"))
        .expect("替换 main 快照");

    let main = load_branch_snapshot(&conn, "repo-1", "main")
        .expect("读取 main")
        .expect("main 应存在");
    let feature = load_branch_snapshot(&conn, "repo-1", "feature")
        .expect("读取 feature")
        .expect("feature 应存在");
    let snapshot_count: i64 = conn
        .query_row("select count(*) from analysis_snapshots", [], |row| {
            row.get(0)
        })
        .expect("读取快照数量");

    assert_eq!(main.analysis_id, "analysis-main-2");
    assert_eq!(feature.analysis_id, "analysis-feature-1");
    assert_eq!(snapshot_count, 2);
}

#[test]
fn records_compact_two_branch_cache_counts_and_database_size() {
    let (dir, conn) = database();
    publish_branch_snapshot(&conn, &snapshot("analysis-main-1", "main", "head-main-1"))
        .expect("发布 main 快照");
    publish_branch_snapshot(
        &conn,
        &snapshot("analysis-feature-1", "feature", "head-feature-1"),
    )
    .expect("发布 feature 快照");
    conn.execute_batch("checkpoint").expect("落盘缓存数据库");

    let counts = [
        "analysis_snapshots",
        "analysis_filter_authors",
        "analysis_filter_globs",
        "analysis_files",
    ]
    .map(|table| {
        conn.query_row(&format!("select count(*) from {table}"), [], |row| {
            row.get::<_, i64>(0)
        })
        .expect("读取缓存表行数")
    });
    let database_size = std::fs::metadata(dir.path().join("index.duckdb"))
        .expect("读取缓存数据库大小")
        .len();

    assert_eq!(counts, [2, 2, 2, 2]);
    assert!(database_size > 0);
}

#[test]
fn failed_replacement_keeps_previous_snapshot() {
    let (_dir, conn) = database();
    let original = snapshot("analysis-main-1", "main", "head-main-1");
    publish_branch_snapshot(&conn, &original).expect("发布旧快照");
    let mut invalid = snapshot("analysis-main-2", "main", "head-main-2");
    invalid.files.push(invalid.files[0].clone());

    let result = publish_branch_snapshot(&conn, &invalid);

    assert!(result.is_err());
    let loaded = load_branch_snapshot(&conn, "repo-1", "main")
        .expect("读取 main")
        .expect("旧快照应保留");
    assert_eq!(loaded.analysis_id, original.analysis_id);
    assert_eq!(loaded.head_commit, original.head_commit);
}

#[test]
fn failed_pruning_rolls_back_candidate_snapshot_publication() {
    let (_dir, conn) = database();
    let original = snapshot("analysis-main-1", "main", "head-main-1");
    publish_branch_snapshot(&conn, &original).expect("发布旧快照");
    conn.execute(
        "insert into commits
         (hash, short_hash, author_name, author_email, author_key, committed_at,
          subject, parent_count, is_merge)
         values ('obsolete', 'obsolete', '测试作者', null, '测试作者',
                 '2026-07-22T00:00:00Z', '废弃提交', 0, false)",
        [],
    )
    .expect("写入废弃提交");
    conn.execute_batch("drop table commit_files")
        .expect("破坏清理依赖表");
    let candidate = snapshot("analysis-main-2", "main", "head-main-2");

    let result = publish_branch_snapshot_and_prune(&conn, &candidate, &[]);

    assert!(result.is_err());
    let loaded = load_branch_snapshot(&conn, "repo-1", "main")
        .expect("读取 main")
        .expect("旧快照应存在");
    assert_eq!(loaded.analysis_id, original.analysis_id);
    assert_eq!(loaded.head_commit, original.head_commit);
}

#[test]
fn successful_snapshot_replacement_prunes_force_moved_old_head() {
    let (_dir, conn) = database();
    insert_index_commit(&conn, "head-main-1");
    insert_index_commit(&conn, "head-main-2");
    publish_branch_snapshot(&conn, &snapshot("analysis-main-1", "main", "head-main-1"))
        .expect("发布旧快照");
    let candidate = snapshot("analysis-main-2", "main", "head-main-2");

    let pruned = publish_branch_snapshot_and_prune(&conn, &candidate, &["head-main-2".to_string()])
        .expect("发布新快照并清理旧 HEAD");

    assert_eq!(pruned, 1);
    let old_exists: bool = conn
        .query_row(
            "select count(*) > 0 from commits where hash = 'head-main-1'",
            [],
            |row| row.get(0),
        )
        .expect("读取旧 HEAD");
    assert!(!old_exists);
    assert_eq!(
        load_branch_snapshot(&conn, "repo-1", "main")
            .expect("读取 main")
            .expect("新快照应存在")
            .analysis_id,
        candidate.analysis_id
    );
}

#[test]
fn stores_normalized_file_analysis_and_replaces_it_atomically() {
    let (_dir, conn) = database();
    publish_branch_snapshot(&conn, &snapshot("analysis-main-1", "main", "head-main-1"))
        .expect("发布项目快照");
    let first = file_analysis("file-analysis-1", "signature-1", "commit-1");
    replace_file_analysis(&conn, &first).expect("保存文件分析");
    let loaded = load_file_analysis(&conn, "analysis-main-1", "src/lib.rs")
        .expect("读取文件分析")
        .expect("文件分析应存在");
    assert_eq!(loaded, first);

    let second = file_analysis("file-analysis-2", "signature-2", "commit-2");
    replace_file_analysis(&conn, &second).expect("替换文件分析");
    let loaded = load_file_analysis(&conn, "analysis-main-1", "src/lib.rs")
        .expect("读取替换后的文件分析")
        .expect("替换后的文件分析应存在");
    assert_eq!(loaded, second);
    let count: i64 = conn
        .query_row("select count(*) from file_analyses", [], |row| row.get(0))
        .expect("读取文件分析数量");
    assert_eq!(count, 1);
}

#[test]
fn failed_file_replacement_keeps_previous_analysis() {
    let (_dir, conn) = database();
    publish_branch_snapshot(&conn, &snapshot("analysis-main-1", "main", "head-main-1"))
        .expect("发布项目快照");
    let original = file_analysis("file-analysis-1", "signature-1", "commit-1");
    replace_file_analysis(&conn, &original).expect("保存文件分析");
    let mut invalid = file_analysis("file-analysis-2", "signature-2", "commit-2");
    invalid.blocks.push(invalid.blocks[0].clone());

    assert!(replace_file_analysis(&conn, &invalid).is_err());
    let loaded = load_file_analysis(&conn, "analysis-main-1", "src/lib.rs")
        .expect("读取文件分析")
        .expect("旧文件分析应保留");
    assert_eq!(loaded, original);
}

#[test]
fn caches_commit_overlay_and_file_refresh_removes_it() {
    let (_dir, conn) = database();
    publish_branch_snapshot(&conn, &snapshot("analysis-main-1", "main", "head-main-1"))
        .expect("发布项目快照");
    let first = file_analysis("file-analysis-1", "signature-1", "commit-1");
    replace_file_analysis(&conn, &first).expect("保存文件分析");
    let overlay = commit_overlay("file-analysis-1");
    replace_commit_overlay(&conn, &overlay).expect("保存提交下钻");

    let loaded = load_commit_overlay(&conn, "file-analysis-1", "commit-1")
        .expect("读取提交下钻")
        .expect("提交下钻应存在");
    assert_eq!(loaded, overlay);

    let second = file_analysis("file-analysis-2", "signature-2", "commit-2");
    replace_file_analysis(&conn, &second).expect("刷新文件分析");
    assert!(
        load_commit_overlay(&conn, "file-analysis-1", "commit-1")
            .expect("读取旧提交下钻")
            .is_none(),
        "文件刷新后旧下钻必须清除"
    );
}

#[test]
fn failed_commit_overlay_replacement_keeps_previous_overlay() {
    let (_dir, conn) = database();
    publish_branch_snapshot(&conn, &snapshot("analysis-main-1", "main", "head-main-1"))
        .expect("发布项目快照");
    replace_file_analysis(
        &conn,
        &file_analysis("file-analysis-1", "signature-1", "commit-1"),
    )
    .expect("保存文件分析");
    let original = commit_overlay("file-analysis-1");
    replace_commit_overlay(&conn, &original).expect("保存提交下钻");
    let mut invalid = original.clone();
    invalid.commit_overlay_id = "overlay-2".to_string();
    invalid.blocks.push(invalid.blocks[0].clone());

    assert!(replace_commit_overlay(&conn, &invalid).is_err());
    let loaded = load_commit_overlay(&conn, "file-analysis-1", "commit-1")
        .expect("读取提交下钻")
        .expect("旧提交下钻应保留");
    assert_eq!(loaded, original);
}

#[test]
fn reports_branch_cache_state_from_head_and_analysis_version() {
    let (_dir, conn) = database();
    assert_eq!(
        branch_cache_state(&conn, "repo-1", "main", "head-main-1", 1).expect("读取空缓存状态"),
        CacheState::Miss
    );

    publish_branch_snapshot(&conn, &snapshot("analysis-main-1", "main", "head-main-1"))
        .expect("发布项目快照");
    assert_eq!(
        branch_cache_state(&conn, "repo-1", "main", "head-main-1", 1).expect("读取命中状态"),
        CacheState::Hit
    );
    assert_eq!(
        branch_cache_state(&conn, "repo-1", "main", "head-main-2", 1).expect("读取 HEAD 过期状态"),
        CacheState::Stale
    );
    assert_eq!(
        branch_cache_state(&conn, "repo-1", "main", "head-main-1", 2)
            .expect("读取算法版本过期状态"),
        CacheState::Stale
    );
}

#[test]
fn persists_only_a_selected_file_that_belongs_to_the_branch_snapshot() {
    let (_dir, conn) = database();
    publish_branch_snapshot(&conn, &snapshot("analysis-main-1", "main", "head-main-1"))
        .expect("发布项目快照");

    update_last_selected_path(&conn, "repo-1", "main", Some("src/lib.rs")).expect("保存选中文件");
    let loaded = load_branch_snapshot(&conn, "repo-1", "main")
        .expect("读取项目快照")
        .expect("项目快照应存在");
    assert_eq!(loaded.last_selected_path.as_deref(), Some("src/lib.rs"));

    assert!(update_last_selected_path(&conn, "repo-1", "main", Some("src/missing.rs")).is_err());
    assert_eq!(
        load_branch_snapshot(&conn, "repo-1", "main")
            .expect("读取项目快照")
            .expect("项目快照应存在")
            .last_selected_path
            .as_deref(),
        Some("src/lib.rs")
    );
}

#[test]
fn explicitly_deletes_all_commit_overlays_for_current_file() {
    let (_dir, conn) = database();
    publish_branch_snapshot(&conn, &snapshot("analysis-main-1", "main", "head-main-1"))
        .expect("发布项目快照");
    replace_file_analysis(
        &conn,
        &file_analysis("file-analysis-1", "signature-1", "commit-1"),
    )
    .expect("保存文件分析");
    replace_commit_overlay(&conn, &commit_overlay("file-analysis-1")).expect("保存提交下钻");

    delete_file_commit_overlays(&conn, "analysis-main-1", "src/lib.rs").expect("删除当前文件下钻");

    assert!(load_commit_overlay(&conn, "file-analysis-1", "commit-1")
        .expect("读取提交下钻")
        .is_none());
}

#[test]
fn cleans_orphaned_cache_rows_without_touching_referenced_data() {
    let (_dir, conn) = database();
    publish_branch_snapshot(&conn, &snapshot("analysis-main-1", "main", "head-main-1"))
        .expect("发布项目快照");
    conn.execute(
        "insert into analysis_filter_authors (analysis_id, ordinal, author_key) values ('orphan', 0, 'orphan@example.com')",
        [],
    )
    .expect("写入孤立缓存行");

    cleanup_orphaned_cache_rows(&conn).expect("清理孤立缓存行");

    let orphan_count: i64 = conn
        .query_row(
            "select count(*) from analysis_filter_authors where analysis_id = 'orphan'",
            [],
            |row| row.get(0),
        )
        .expect("读取孤立缓存行数量");
    let referenced_count: i64 = conn
        .query_row(
            "select count(*) from analysis_filter_authors where analysis_id = 'analysis-main-1'",
            [],
            |row| row.get(0),
        )
        .expect("读取有效缓存行数量");
    assert_eq!(orphan_count, 0);
    assert_eq!(referenced_count, 1);
}

fn database() -> (tempfile::TempDir, duckdb::Connection) {
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
    (dir, conn)
}

fn insert_index_commit(conn: &duckdb::Connection, hash: &str) {
    conn.execute(
        "insert into commits
         (hash, short_hash, author_name, author_email, author_key, committed_at,
          subject, parent_count, is_merge)
         values (?, ?, '测试作者', null, '测试作者', '2026-07-22T00:00:00Z',
                 '测试提交', 0, false)",
        duckdb::params![hash, hash],
    )
    .expect("写入索引提交");
}

fn snapshot(analysis_id: &str, branch: &str, head: &str) -> CachedAnalysisSnapshot {
    CachedAnalysisSnapshot {
        analysis_id: analysis_id.to_string(),
        repo_id: "repo-1".to_string(),
        branch: branch.to_string(),
        base_commit: "base-1".to_string(),
        head_commit: head.to_string(),
        start_at: Some("2026-07-01T00:00:00Z".to_string()),
        end_at: Some("2026-07-21T00:00:00Z".to_string()),
        author_query: Some("alice".to_string()),
        message_query: Some("perf".to_string()),
        filter_fingerprint: format!("filter-{branch}"),
        analysis_version: 1,
        started_at: "2026-07-21T00:00:00Z".to_string(),
        completed_at: "2026-07-21T00:00:01Z".to_string(),
        elapsed_ms: 1000,
        last_selected_path: None,
        author_keys: vec!["alice@example.com".to_string()],
        globs: vec!["src/**/*.rs".to_string()],
        files: vec![CachedAnalysisFile {
            path: "src/lib.rs".to_string(),
            old_path: None,
            status: 1,
            additions: 10,
            deletions: 2,
            is_binary: false,
            is_previewable: true,
            old_blob_id: Some("old-blob".to_string()),
            new_blob_id: Some("new-blob".to_string()),
        }],
    }
}

fn file_analysis(file_analysis_id: &str, signature: &str, commit_hash: &str) -> CachedFileAnalysis {
    CachedFileAnalysis {
        file_analysis_id: file_analysis_id.to_string(),
        analysis_id: "analysis-main-1".to_string(),
        path: "src/lib.rs".to_string(),
        resolved_encoding: 0,
        block_signature: signature.to_string(),
        analysis_version: 1,
        content_elapsed_ms: 10,
        attribution_elapsed_ms: 20,
        completed_at: "2026-07-21T00:00:01Z".to_string(),
        blocks: vec![CachedFileBlock {
            ordinal: 0,
            old_start: 1,
            old_end: 2,
            new_start: 1,
            new_end: 3,
            change_type: 2,
            confidence: Some(0),
            warning_flags: 0,
            commits: vec![CachedBlockCommit {
                commit_hash: commit_hash.to_string(),
                matched_by_filter: true,
                attribution_method: Some(0),
                touched_ranges: vec![CachedTouchedRange {
                    old_start: Some(1),
                    old_end: Some(2),
                    new_start: Some(1),
                    new_end: Some(3),
                }],
                merge_hashes: vec!["merge-1".to_string()],
            }],
        }],
    }
}

fn commit_overlay(file_analysis_id: &str) -> CachedCommitOverlay {
    CachedCommitOverlay {
        commit_overlay_id: "overlay-1".to_string(),
        file_analysis_id: file_analysis_id.to_string(),
        commit_hash: "commit-1".to_string(),
        parent_hash: "parent-1".to_string(),
        historical_path: "src/lib.rs".to_string(),
        old_blob_id: Some("parent-blob".to_string()),
        new_blob_id: Some("commit-blob".to_string()),
        resolved_encoding: 0,
        analysis_version: 1,
        elapsed_ms: 15,
        completed_at: "2026-07-21T00:00:02Z".to_string(),
        blocks: vec![CachedCommitOverlayBlock {
            ordinal: 0,
            old_start: 1,
            old_end: 1,
            new_start: 1,
            new_end: 2,
            change_type: 2,
        }],
    }
}

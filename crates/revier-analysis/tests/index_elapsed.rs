mod fixtures;

use std::process::Command;
use std::time::Instant;

use serde_json::Value;
use tempfile::tempdir;

#[test]
fn index_output_and_run_record_share_end_to_end_elapsed_time() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let wall_started = Instant::now();

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "build",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--branch",
            "main",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 index build");
    let wall_elapsed_ms = wall_started.elapsed().as_millis() as u64;

    assert!(
        output.status.success(),
        "index build 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("解析 index build 输出");
    let output_elapsed_ms = value["elapsedMs"].as_u64().expect("输出应包含耗时");

    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");
    let (status, stored_elapsed_ms): (String, Option<i64>) = conn
        .query_row(
            "select status, elapsed_ms from index_runs order by started_at desc limit 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("读取索引运行记录");

    assert_eq!(status, "completed");
    assert_eq!(stored_elapsed_ms, Some(output_elapsed_ms as i64));
    assert!(
        output_elapsed_ms <= wall_elapsed_ms,
        "内部耗时不应超过进程外部计时：内部 {output_elapsed_ms}ms，外部 {wall_elapsed_ms}ms"
    );
}

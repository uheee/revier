mod fixtures;

#[test]
fn analyzes_changed_files_and_overlay_from_rust_only() {
    let fixture = fixtures::linear();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    build_index(&fixture, &db_path);
    let mut query_request = fixture.query_files_request();
    query_request.db = Some(db_path);

    let files = revier_analysis::api::query_files(query_request).expect("Rust 文件列表查询失败");
    assert!(!files.files.is_empty());
    assert_eq!(files.files[0].path, "src/app.txt");

    let overlay = revier_analysis::api::file_overlay(fixture.file_overlay_args("src/app.txt"))
        .expect("Rust overlay 查询失败");
    assert!(!overlay.overlay.blocks.is_empty());
}

fn build_index(fixture: &fixtures::FixtureRepo, db_path: &std::path::Path) {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
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

    assert!(
        output.status.success(),
        "index build 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

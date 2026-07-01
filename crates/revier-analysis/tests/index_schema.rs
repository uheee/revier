use tempfile::tempdir;

#[test]
fn initializes_schema_version_one_tables() {
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

    let version =
        revier_analysis::index::schema::read_schema_version(&conn).expect("读取 schema version");
    assert_eq!(version, Some(1));

    let tables = revier_analysis::index::schema::list_tables(&conn).expect("读取表列表");
    assert!(tables.contains(&"metadata".to_string()));
    assert!(tables.contains(&"commits".to_string()));
    assert!(tables.contains(&"commit_parents".to_string()));
    assert!(tables.contains(&"commit_files".to_string()));
    assert!(tables.contains(&"index_runs".to_string()));
}

#[test]
fn reports_incompatible_schema_version() {
    let dir = tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");

    conn.execute_batch(
        "create table metadata (key text primary key, value text not null);
         insert into metadata values ('schema_version', '999');",
    )
    .expect("写入不兼容版本");

    let result = revier_analysis::index::migrations::ensure_compatible_schema(&conn);
    assert!(matches!(
        result,
        Err(revier_analysis::error::AppError::SchemaIncompatible(_))
    ));
}

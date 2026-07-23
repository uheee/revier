use revier_analysis::index::connection::DatabaseRegistry;

#[test]
fn registry_connections_share_one_database_instance() {
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let registry = DatabaseRegistry::default();

    let first = registry.connect(&db_path).expect("创建第一个共享连接");
    first
        .execute_batch(
            "create table values_table (value integer); insert into values_table values (1);",
        )
        .expect("初始化测试表");

    let second = registry.connect(&db_path).expect("创建第二个共享连接");
    second
        .execute("insert into values_table values (?)", [2])
        .expect("通过第二个连接写入");

    let values = first
        .query_row("select sum(value) from values_table", [], |row| {
            row.get::<_, i64>(0)
        })
        .expect("通过第一个连接读取");
    assert_eq!(values, 3);
}

#[test]
fn registry_keeps_root_instance_alive_after_clone_is_dropped() {
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let registry = DatabaseRegistry::default();

    {
        let connection = registry.connect(&db_path).expect("创建共享连接");
        connection
            .execute_batch("create table marker (value text); insert into marker values ('ready');")
            .expect("写入标记");
    }

    let connection = registry.connect(&db_path).expect("重新创建共享连接");
    let marker = connection
        .query_row("select value from marker", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("读取标记");
    assert_eq!(marker, "ready");
}

#[test]
fn registry_keeps_different_database_paths_isolated() {
    let dir = tempfile::tempdir().expect("创建临时目录");
    let first_path = dir.path().join("first.duckdb");
    let second_path = dir.path().join("second.duckdb");
    let registry = DatabaseRegistry::default();

    let first = registry.connect(&first_path).expect("打开第一个数据库");
    first
        .execute_batch("create table marker (value text); insert into marker values ('first');")
        .expect("写入第一个数据库");
    let second = registry.connect(&second_path).expect("打开第二个数据库");
    second
        .execute_batch("create table marker (value text); insert into marker values ('second');")
        .expect("写入第二个数据库");

    let first_marker = first
        .query_row("select value from marker", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("读取第一个数据库");
    let second_marker = second
        .query_row("select value from marker", [], |row| {
            row.get::<_, String>(0)
        })
        .expect("读取第二个数据库");
    assert_eq!(first_marker, "first");
    assert_eq!(second_marker, "second");
}

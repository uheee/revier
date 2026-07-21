use std::fs;
use tempfile::tempdir;

#[test]
fn exports_typescript_bindings_without_recoverable() {
    let dir = tempdir().expect("创建临时目录失败");
    let output = dir.path().join("bindings.ts");

    revier_analysis::bindings::export_typescript_bindings(&output)
        .expect("导出 TypeScript bindings 失败");

    let content = fs::read_to_string(output).expect("读取 bindings 失败");
    assert!(content.contains("export type AppError"));
    assert!(content.contains("code: string"));
    assert!(content.contains("message: string"));
    assert!(content.contains("detail?: string"));
    assert!(!content.contains("recoverable"));
    assert!(!content.contains("| null"));
}

#[test]
fn exports_operation_progress_bindings() {
    let dir = tempdir().expect("创建临时目录失败");
    let output = dir.path().join("bindings.ts");

    revier_analysis::bindings::export_typescript_bindings(&output)
        .expect("导出 TypeScript bindings 失败");

    let content = fs::read_to_string(output).expect("读取 bindings 失败");
    assert!(content.contains("export type OperationKind"));
    assert!(content.contains("export type OperationStage"));
    assert!(content.contains("export type OperationProgressSnapshot"));
    assert!(content.contains("elapsedMs: number"));
    assert!(content.contains("cacheState: CacheState"));
}

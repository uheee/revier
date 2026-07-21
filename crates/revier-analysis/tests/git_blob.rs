mod fixtures;

use revier_analysis::git::blob::{read_blob_at_commit, read_blob_by_id};
use revier_analysis::git::repository::open_repository;

#[test]
fn returns_none_when_intermediate_path_component_is_blob() {
    let fixture = fixtures::binary_change();
    let repo = open_repository(fixture.repo.path()).expect("打开仓库");

    let blob = read_blob_at_commit(&repo, &fixture.head, "README.md/child").expect("读取 blob");

    assert!(blob.is_none());
}

#[test]
fn reads_blob_directly_by_cached_id_and_reports_stable_cache_invalidation() {
    let fixture = fixtures::binary_change();
    let repo = open_repository(fixture.repo.path()).expect("打开仓库");
    let change =
        revier_analysis::git::diff::range_file_tree_changes(&repo, &fixture.base, &fixture.head)
            .expect("读取树差异")
            .into_iter()
            .find(|change| change.new_blob_id.is_some())
            .expect("应存在新侧 Blob");
    let blob_id = change.new_blob_id.expect("新侧 Blob ID 应存在");

    let direct = read_blob_by_id(&repo, &blob_id).expect("按 Blob ID 读取内容");
    let by_path = read_blob_at_commit(&repo, &fixture.head, &change.path)
        .expect("按提交路径读取内容")
        .expect("提交路径内容应存在");
    assert_eq!(direct, by_path);

    let error = read_blob_by_id(&repo, "0000000000000000000000000000000000000000")
        .expect_err("缺失 Blob 应报告缓存失效");
    assert!(matches!(
        error,
        revier_analysis::error::AppError::CacheInvalid(_)
    ));
}

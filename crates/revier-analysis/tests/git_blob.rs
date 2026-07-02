mod fixtures;

use revier_analysis::git::blob::read_blob_at_commit;
use revier_analysis::git::repository::open_repository;

#[test]
fn returns_none_when_intermediate_path_component_is_blob() {
    let fixture = fixtures::binary_change();
    let repo = open_repository(fixture.repo.path()).expect("打开仓库");

    let blob = read_blob_at_commit(&repo, &fixture.head, "README.md/child").expect("读取 blob");

    assert!(blob.is_none());
}

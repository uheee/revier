use std::fs;
use std::path::Path;

#[test]
fn production_source_does_not_spawn_git_process() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = crate_root.join("src");
    let mut violations = Vec::new();
    scan_rs_files(&src, &mut violations);

    assert!(
        violations.is_empty(),
        "src/ must not spawn git processes: {:?}",
        violations
    );
}

fn scan_rs_files(path: &Path, violations: &mut Vec<String>) {
    for entry in fs::read_dir(path).expect("read directory") {
        let entry = entry.expect("read directory entry");
        let path = entry.path();
        if path.is_dir() {
            scan_rs_files(&path, violations);
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read source file");
        if source.contains("Command::new(\"git\")")
            || source.contains("Command::new('git')")
            || source.contains("std::process::Command")
        {
            violations.push(path.display().to_string());
        }
    }
}

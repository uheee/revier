#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub struct FixtureRepo {
    pub name: &'static str,
    pub repo: TempDir,
    pub base: String,
    pub head: String,
}

impl FixtureRepo {
    pub fn query_files_request(&self) -> revier_analysis::api::QueryFilesRequest {
        revier_analysis::api::QueryFilesRequest {
            repo: self.repo.path().to_path_buf(),
            db: None,
            base: self.base.clone(),
            head: self.head.clone(),
            branch: "main".to_string(),
            authors: Vec::new(),
            author_query: None,
            message: None,
            since: None,
            until: None,
            globs: Vec::new(),
        }
    }

    pub fn file_overlay_args(
        &self,
        file: impl Into<String>,
    ) -> revier_analysis::cli::FileOverlayArgs {
        revier_analysis::cli::FileOverlayArgs {
            common: revier_analysis::cli::OverlayCommonArgs {
                repo: self.repo.path().to_path_buf(),
                db: None,
                base: self.base.clone(),
                head: self.head.clone(),
                branch: "main".to_string(),
                globs: Vec::new(),
                authors: Vec::new(),
                author_query: None,
                message: None,
                require_index: false,
                format: revier_analysis::cli::OutputFormat::Json,
                pretty: false,
            },
            file: file.into(),
            encoding: "auto".to_string(),
        }
    }
}

pub fn linear() -> FixtureRepo {
    let repo = init_repo("linear");
    write_file(repo.path(), "src/app.txt", "one\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: initial"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "one\ntwo\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add second line"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "linear",
        repo,
        base,
        head,
    }
}

pub fn linear_deletion() -> FixtureRepo {
    let repo = init_repo("linear-deletion");
    write_file(repo.path(), "src/app.txt", "keep\ndelete me\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "keep\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "fix: delete line"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "linear-deletion",
        repo,
        base,
        head,
    }
}

pub fn changed_then_restored() -> FixtureRepo {
    let repo = init_repo("changed-then-restored");
    write_file(repo.path(), "src/app.txt", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "base\ntemporary\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: temporary change"]);

    write_file(repo.path(), "src/app.txt", "base\n");
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "revert: restore base content"],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "changed-then-restored",
        repo,
        base,
        head,
    }
}

pub fn added_and_deleted_text_files() -> FixtureRepo {
    let repo = init_repo("added-and-deleted-text-files");
    write_file(repo.path(), "src/deleted.txt", "old one\nold two\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base file"]);
    let base = rev_parse(repo.path(), "HEAD");

    fs::remove_file(repo.path().join("src/deleted.txt")).expect("删除旧文本文件");
    write_file(
        repo.path(),
        "src/added.txt",
        "new one\nnew two\nnew three\n",
    );
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: replace text file"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "added-and-deleted-text-files",
        repo,
        base,
        head,
    }
}

pub fn replacement_hunk_deletion() -> FixtureRepo {
    let repo = init_repo("replacement-hunk-deletion");
    write_file(repo.path(), "src/app.txt", "keep\ndelete me\ntail\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "keep\nreplacement\ntail\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "fix: replace deleted line"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "replacement-hunk-deletion",
        repo,
        base,
        head,
    }
}

pub fn stale_modified_deletion_then_final_change() -> FixtureRepo {
    let repo = init_repo("stale-modified-deletion");
    write_file(repo.path(), "src/app.txt", "foo\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base foo"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "bar\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "fix: foo to bar"]);

    write_file(repo.path(), "src/app.txt", "baz\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "fix: bar to baz"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "stale-modified-deletion",
        repo,
        base,
        head,
    }
}

pub fn linear_with_authors() -> FixtureRepo {
    let repo = init_repo("linear-authors");
    write_file(repo.path(), "src/app.txt", "base\n");
    git_with_author(repo.path(), ["add", "."], "Base", "base@example.com", None);
    git_with_author(
        repo.path(),
        ["commit", "-m", "feat: base"],
        "Base",
        "base@example.com",
        Some("2026-05-01T00:00:00Z"),
    );
    let base = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "base\nalice\n");
    git_with_author(
        repo.path(),
        ["add", "."],
        "Alice",
        "alice@example.com",
        None,
    );
    git_with_author(
        repo.path(),
        ["commit", "-m", "feat: alice change"],
        "Alice",
        "alice@example.com",
        Some("2026-05-02T00:00:00Z"),
    );

    write_file(repo.path(), "src/app.txt", "base\nalice\nbob\n");
    git_with_author(repo.path(), ["add", "."], "Bob", "bob@example.com", None);
    git_with_author(
        repo.path(),
        ["commit", "-m", "fix: bob change"],
        "Bob",
        "bob@example.com",
        Some("2026-05-03T00:00:00Z"),
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "linear-authors",
        repo,
        base,
        head,
    }
}

pub fn separate_files_with_authors() -> FixtureRepo {
    let repo = init_repo("separate-files-authors");
    write_file(repo.path(), "src/app.txt", "base\n");
    write_file(repo.path(), "docs/notes.txt", "base\n");
    git_with_author(repo.path(), ["add", "."], "Base", "base@example.com", None);
    git_with_author(
        repo.path(),
        ["commit", "-m", "feat: base"],
        "Base",
        "base@example.com",
        Some("2026-05-01T00:00:00Z"),
    );
    let base = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "base\nalice\n");
    git_with_author(
        repo.path(),
        ["add", "."],
        "Alice",
        "alice@example.com",
        None,
    );
    git_with_author(
        repo.path(),
        ["commit", "-m", "feat: alice app"],
        "Alice",
        "alice@example.com",
        Some("2026-05-02T00:00:00Z"),
    );

    write_file(repo.path(), "docs/notes.txt", "base\nbob\n");
    git_with_author(repo.path(), ["add", "."], "Bob", "bob@example.com", None);
    git_with_author(
        repo.path(),
        ["commit", "-m", "docs: bob notes"],
        "Bob",
        "bob@example.com",
        Some("2026-05-03T00:00:00Z"),
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "separate-files-authors",
        repo,
        base,
        head,
    }
}

pub fn merge_conflict() -> FixtureRepo {
    let repo = init_repo("merge-conflict");
    write_file(repo.path(), "src/app.txt", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "feature"]);
    write_file(repo.path(), "src/app.txt", "base\nfeature\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: feature line"]);

    git(repo.path(), ["checkout", "main"]);
    write_file(repo.path(), "src/app.txt", "base\nmain\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: main line"]);

    git_expect_failure(repo.path(), ["merge", "feature"]);
    write_file(repo.path(), "src/app.txt", "base\nmain\nfeature\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "merge: resolve conflict"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "merge-conflict",
        repo,
        base,
        head,
    }
}

pub fn deletion_merge() -> FixtureRepo {
    let repo = init_repo("deletion-merge");
    write_file(repo.path(), "src/app.txt", "keep\ndelete me\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "feature"]);
    write_file(repo.path(), "src/app.txt", "keep\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "fix: delete line"]);

    git(repo.path(), ["checkout", "main"]);
    git(
        repo.path(),
        ["merge", "--no-ff", "feature", "-m", "merge: delete feature"],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "deletion-merge",
        repo,
        base,
        head,
    }
}

pub fn duplicate_deletions_same_text() -> FixtureRepo {
    let repo = init_repo("duplicate-deletions-same-text");
    write_file(repo.path(), "src/app.txt", "dup\nkeep\ndup\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: duplicate base"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "keep\ndup\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "fix: delete first dup"]);

    write_file(repo.path(), "src/app.txt", "keep\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "fix: delete second dup"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "duplicate-deletions-same-text",
        repo,
        base,
        head,
    }
}

pub fn multi_parent_ambiguous() -> FixtureRepo {
    let repo = init_repo("multi-parent-ambiguous");
    write_file(repo.path(), "src/app.txt", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "left"]);
    write_file(repo.path(), "src/app.txt", "base\nshared\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: left shared"]);

    git(repo.path(), ["checkout", "main"]);
    git(repo.path(), ["checkout", "-b", "right"]);
    write_file(repo.path(), "src/app.txt", "base\nshared\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: right shared"]);

    git(repo.path(), ["checkout", "main"]);
    let tree = rev_parse(repo.path(), "left^{tree}");
    let left = rev_parse(repo.path(), "left");
    let right = rev_parse(repo.path(), "right");
    let head = git_commit_tree(
        repo.path(),
        &tree,
        [&base, &left, &right],
        "merge: ambiguous shared",
    );
    git(repo.path(), ["reset", "--hard", &head]);

    FixtureRepo {
        name: "multi-parent-ambiguous",
        repo,
        base,
        head,
    }
}

pub fn merge_adds_duplicate_text_with_parent_match_elsewhere() -> FixtureRepo {
    let repo = init_repo("merge-duplicate-text-window");
    write_file(repo.path(), "src/app.txt", "shared\nbase\n");
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "feat: base with shared header"],
    );
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "docs"]);
    write_file(repo.path(), "docs/notes.txt", "docs only\n");
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "docs: parent with same text elsewhere"],
    );

    git(repo.path(), ["checkout", "main"]);
    git(repo.path(), ["merge", "--no-ff", "--no-commit", "docs"]);
    write_file(repo.path(), "src/app.txt", "shared\nbase\nshared\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "merge: add duplicate shared"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "merge-duplicate-text-window",
        repo,
        base,
        head,
    }
}

pub fn unrelated_merge_contains_same_block_text() -> FixtureRepo {
    let repo = init_repo("unrelated-merge-same-text");
    write_file(repo.path(), "src/app.txt", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "base\nshared\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add shared line"]);

    git(repo.path(), ["checkout", "-b", "docs"]);
    write_file(repo.path(), "docs/notes.txt", "docs only\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "docs: add notes"]);

    git(repo.path(), ["checkout", "main"]);
    git(
        repo.path(),
        ["merge", "--no-ff", "docs", "-m", "merge: docs only"],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "unrelated-merge-same-text",
        repo,
        base,
        head,
    }
}

pub fn merge_source_followed_by_unrelated_commit() -> FixtureRepo {
    let repo = init_repo("merge-source-followed-by-unrelated-commit");
    write_file(repo.path(), "src/app.txt", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "feature"]);
    write_file(repo.path(), "src/app.txt", "base\nfeature line\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add feature line"]);

    git(repo.path(), ["checkout", "main"]);
    write_file(repo.path(), "docs/notes.txt", "docs only\n");
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "chore: unrelated docs change"],
    );

    git(
        repo.path(),
        [
            "merge",
            "--no-ff",
            "feature",
            "-m",
            "merge: bring feature line",
        ],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "merge-source-followed-by-unrelated-commit",
        repo,
        base,
        head,
    }
}

pub fn nested_merge_source() -> FixtureRepo {
    let repo = init_repo("nested-merge-source");
    write_file(repo.path(), "src/app.txt", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "feature"]);
    write_file(repo.path(), "src/app.txt", "base\nA\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add A line"]);

    git(repo.path(), ["checkout", "main"]);
    write_file(repo.path(), "docs/notes.txt", "main prep\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "chore: main prep"]);

    git(repo.path(), ["checkout", "-b", "integration"]);
    git(
        repo.path(),
        [
            "merge",
            "--no-ff",
            "feature",
            "-m",
            "merge: feature into integration",
        ],
    );

    git(repo.path(), ["checkout", "main"]);
    write_file(repo.path(), "docs/notes.txt", "final prep\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "chore: final prep"]);

    git(
        repo.path(),
        [
            "merge",
            "--no-ff",
            "integration",
            "-m",
            "merge: integrate feature",
        ],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "nested-merge-source",
        repo,
        base,
        head,
    }
}

pub fn merge_only_differs_from_second_parent() -> FixtureRepo {
    let repo = init_repo("merge-only-second-parent-diff");
    write_file(repo.path(), "src/app.txt", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "feature"]);
    write_file(repo.path(), "src/app.txt", "base\nfeature line\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add feature line"]);

    git(repo.path(), ["checkout", "main"]);
    write_file(repo.path(), "docs/notes.txt", "main prep\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "chore: main prep"]);

    git(
        repo.path(),
        [
            "merge",
            "--no-ff",
            "feature",
            "-m",
            "merge: second parent only change",
        ],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "merge-only-second-parent-diff",
        repo,
        base,
        head,
    }
}

pub fn ambiguous_nested_merge_sources() -> FixtureRepo {
    let repo = init_repo("ambiguous-nested-merge-sources");
    write_file(repo.path(), "src/app.txt", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "left"]);
    write_file(repo.path(), "src/app.txt", "base\nshared\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: left path"]);
    let left = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "main"]);
    git(repo.path(), ["checkout", "-b", "right", "main"]);
    write_file(repo.path(), "src/app.txt", "base\nshared\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: right path"]);
    let right = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "main"]);
    write_file(repo.path(), "docs/notes.txt", "main keep\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "chore: main prep"]);

    git(repo.path(), ["checkout", "-b", "integration", &base]);
    let m1 = git_commit_tree(
        repo.path(),
        &rev_parse(repo.path(), &format!("{}^{{tree}}", right)),
        [left.as_str(), right.as_str()],
        "merge: ambiguous nested sources",
    );
    git(repo.path(), ["reset", "--hard", &m1]);

    git(repo.path(), ["checkout", "main"]);
    git(
        repo.path(),
        [
            "merge",
            "--no-ff",
            "integration",
            "-m",
            "merge: lift ambiguous merge",
        ],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "ambiguous-nested-merge-sources",
        repo,
        base,
        head,
    }
}

pub fn merge_deletion_source_followed_by_unrelated_commit() -> FixtureRepo {
    let repo = init_repo("merge-deletion-source-followed-by-unrelated-commit");
    write_file(repo.path(), "src/app.txt", "keep\ndelete me\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "feature"]);
    write_file(repo.path(), "src/app.txt", "keep\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "fix: delete line"]);

    git(repo.path(), ["checkout", "main"]);
    write_file(repo.path(), "docs/notes.txt", "docs keep\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "chore: unrelated docs"]);

    git(
        repo.path(),
        [
            "merge",
            "--no-ff",
            "feature",
            "-m",
            "merge: delete from feature",
        ],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "merge-deletion-source-followed-by-unrelated-commit",
        repo,
        base,
        head,
    }
}

pub fn rename_merge() -> FixtureRepo {
    let repo = init_repo("rename-merge");
    write_file(repo.path(), "src/old.txt", "alpha\nbeta\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add old file"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "feature"]);
    git(repo.path(), ["mv", "src/old.txt", "src/new.txt"]);
    write_file(repo.path(), "src/new.txt", "alpha\nbeta\nfeature\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: rename and edit"]);

    git(repo.path(), ["checkout", "main"]);
    git(
        repo.path(),
        ["merge", "--no-ff", "feature", "-m", "merge: feature rename"],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "rename-merge",
        repo,
        base,
        head,
    }
}

pub fn pure_rename() -> FixtureRepo {
    let repo = init_repo("pure-rename");
    write_file(repo.path(), "src/old.txt", "alpha\nbeta\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add old file"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["mv", "src/old.txt", "src/new.txt"]);
    git(repo.path(), ["commit", "-m", "refactor: rename file"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "pure-rename",
        repo,
        base,
        head,
    }
}

pub fn rename_then_unrelated() -> FixtureRepo {
    let repo = init_repo("rename-unrelated");
    write_file(repo.path(), "src/old.txt", "alpha\nbeta\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add old file"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["mv", "src/old.txt", "src/new.txt"]);
    write_file(repo.path(), "src/new.txt", "alpha\nbeta\nfeature\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: rename and edit"]);

    write_file(repo.path(), "docs/notes.txt", "unrelated\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "chore: unrelated docs"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "rename-unrelated",
        repo,
        base,
        head,
    }
}

pub fn rename_then_reused_old_path() -> FixtureRepo {
    let repo = init_repo("rename-reused-old-path");
    write_file(repo.path(), "src/old.txt", "alpha\nbeta\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add old file"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["mv", "src/old.txt", "src/new.txt"]);
    write_file(repo.path(), "src/new.txt", "alpha\nbeta\nfeature\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: rename and edit"]);

    write_file(repo.path(), "src/old.txt", "one\ntwo\nunrelated\n");
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "chore: edit recreated old path same line"],
    );

    fs::remove_file(repo.path().join("src/old.txt")).expect("删除复用旧路径文件");
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "chore: remove recreated old path"],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "rename-reused-old-path",
        repo,
        base,
        head,
    }
}

pub fn rename_delete_then_reused_old_path_delete() -> FixtureRepo {
    let repo = init_repo("rename-delete-reused-old-path");
    write_file(
        repo.path(),
        "src/old.txt",
        "keep\nstay\nanother\ndelete me\n",
    );
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add old file"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["mv", "src/old.txt", "src/new.txt"]);
    write_file(repo.path(), "src/new.txt", "keep\nstay\nanother\n");
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "fix: rename and delete tracked line"],
    );

    write_file(
        repo.path(),
        "src/old.txt",
        "keep\nstay\nanother\ndelete me\n",
    );
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "chore: recreate old path"]);

    fs::remove_file(repo.path().join("src/old.txt")).expect("删除复用旧路径文件");
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "chore: delete reused old path"],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "rename-delete-reused-old-path",
        repo,
        base,
        head,
    }
}

pub fn multi_hop_rename_delete() -> FixtureRepo {
    let repo = init_repo("multi-hop-rename-delete");
    write_file(repo.path(), "src/a.txt", "keep\nstay\nanother\ndelete me\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add a"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["mv", "src/a.txt", "src/b.txt"]);
    git(repo.path(), ["commit", "-m", "refactor: rename a to b"]);

    write_file(repo.path(), "src/b.txt", "keep\nstay\nanother\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "fix: delete line on b"]);

    git(repo.path(), ["mv", "src/b.txt", "src/c.txt"]);
    git(repo.path(), ["commit", "-m", "refactor: rename b to c"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "multi-hop-rename-delete",
        repo,
        base,
        head,
    }
}

pub fn binary_change() -> FixtureRepo {
    let repo = init_repo("binary-change");
    write_file(repo.path(), "README.md", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    let full_path = repo.path().join("assets/logo.bin");
    fs::create_dir_all(full_path.parent().expect("binary parent")).expect("创建 binary 目录");
    fs::write(&full_path, [0_u8, 159, 146, 150, 0, 1, 2, 3]).expect("写入 binary");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add binary"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "binary-change",
        repo,
        base,
        head,
    }
}

pub fn gb18030_change() -> FixtureRepo {
    let repo = init_repo("gb18030-change");
    write_file(repo.path(), ".gitattributes", "* -text\n");
    write_bytes(
        repo.path(),
        "src/app.txt",
        encoding_rs::GB18030.encode("你好\n").0.as_ref(),
    );
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: 添加 GB18030 文本"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_bytes(
        repo.path(),
        "src/app.txt",
        encoding_rs::GB18030.encode("你好，世界\n").0.as_ref(),
    );
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: 修改 GB18030 文本"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "gb18030-change",
        repo,
        base,
        head,
    }
}

pub fn utf16le_change() -> FixtureRepo {
    let repo = init_repo("utf16le-change");
    write_file(repo.path(), ".gitattributes", "* -text\n");
    write_bytes(repo.path(), "src/app.txt", &utf16le_bom("你好\n"));
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: 添加 UTF-16LE 文本"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_bytes(repo.path(), "src/app.txt", &utf16le_bom("你好，世界\n"));
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: 修改 UTF-16LE 文本"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "utf16le-change",
        repo,
        base,
        head,
    }
}

pub fn conflicting_utf16_bom_change() -> FixtureRepo {
    let repo = init_repo("conflicting-utf16-bom-change");
    write_file(repo.path(), ".gitattributes", "* -text\n");
    write_bytes(repo.path(), "src/app.txt", &utf16be_bom("旧\n"));
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: 添加 UTF-16BE 文本"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_bytes(repo.path(), "src/app.txt", &utf16le_bom("新\n"));
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: 改为 UTF-16LE 文本"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "conflicting-utf16-bom-change",
        repo,
        base,
        head,
    }
}

pub fn utf16_bom_binary_change() -> FixtureRepo {
    let repo = init_repo("utf16-bom-binary-change");
    write_file(repo.path(), ".gitattributes", "* -text\n");
    write_bytes(repo.path(), "src/app.txt", &utf16le_bom("正常\n"));
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "feat: 添加正常 UTF-16LE 文本"],
    );
    let base = rev_parse(repo.path(), "HEAD");

    write_bytes(repo.path(), "src/app.txt", &utf16le_bom("\u{1}\u{2}\u{3}A"));
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "test: 写入 UTF-16 控制字符"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "utf16-bom-binary-change",
        repo,
        base,
        head,
    }
}

pub fn manual_utf16_binary_change() -> FixtureRepo {
    let repo = init_repo("manual-utf16-binary-change");
    write_file(repo.path(), ".gitattributes", "* -text\n");
    write_bytes(repo.path(), "src/app.txt", &utf16le("正常\n"));
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "feat: 添加无 BOM UTF-16LE 文本"],
    );
    let base = rev_parse(repo.path(), "HEAD");

    write_bytes(repo.path(), "src/app.txt", &utf16le("A\0B"));
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "test: 写入 Unicode NUL"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "manual-utf16-binary-change",
        repo,
        base,
        head,
    }
}

pub fn exact_text_shapes() -> FixtureRepo {
    let repo = init_repo("exact-text-shapes");
    write_file(repo.path(), ".gitattributes", "* -text\n");
    write_bytes(repo.path(), "src/crlf.txt", b"one\r\n");
    write_bytes(repo.path(), "src/trailing.txt", b"old");
    write_bytes(repo.path(), "src/empty.txt", b"not empty\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: 添加文本形态夹具"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_bytes(repo.path(), "src/crlf.txt", b"one\r\ntwo\r\n");
    write_bytes(repo.path(), "src/trailing.txt", b"new\n");
    write_bytes(repo.path(), "src/empty.txt", b"");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: 修改文本形态夹具"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "exact-text-shapes",
        repo,
        base,
        head,
    }
}

fn init_repo(name: &'static str) -> TempDir {
    let repo = tempfile::Builder::new()
        .prefix(&format!("revier-{name}-"))
        .tempdir()
        .expect("create temp repo");

    git(repo.path(), ["init", "-b", "main"]);
    git(repo.path(), ["config", "user.name", "Fixture Author"]);
    git(repo.path(), ["config", "user.email", "fixture@example.com"]);
    repo
}

fn write_file(repo: &Path, path: &str, content: &str) {
    write_bytes(repo, path, content.as_bytes());
}

fn write_bytes(repo: &Path, path: &str, content: &[u8]) {
    let full_path = repo.join(path);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).expect("create parent directory");
    }
    fs::write(full_path, content).expect("write fixture file");
}

fn utf16le_bom(content: &str) -> Vec<u8> {
    let mut bytes = vec![0xff, 0xfe];
    bytes.extend(utf16le(content));
    bytes
}

fn utf16le(content: &str) -> Vec<u8> {
    content.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn utf16be_bom(content: &str) -> Vec<u8> {
    let mut bytes = vec![0xfe, 0xff];
    bytes.extend(content.encode_utf16().flat_map(u16::to_be_bytes));
    bytes
}

fn rev_parse(repo: &Path, rev: &str) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", rev])
        .output()
        .expect("run git rev-parse");

    assert!(
        output.status.success(),
        "git rev-parse failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf8 hash")
        .trim()
        .to_string()
}

fn git_commit_tree<const N: usize>(
    repo: &Path,
    tree: &str,
    parents: [&str; N],
    message: &str,
) -> String {
    let mut args = vec!["commit-tree", tree];
    for parent in parents {
        args.push("-p");
        args.push(parent);
    }
    args.push("-m");
    args.push(message);

    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("run git commit-tree");

    assert!(
        output.status.success(),
        "git commit-tree failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf8 hash")
        .trim()
        .to_string()
}

fn git<const N: usize>(repo: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("run git command");

    assert!(
        output.status.success(),
        "git command failed: {}\nstdout: {}\nstderr: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_with_author<const N: usize>(
    repo: &Path,
    args: [&str; N],
    name: &str,
    email: &str,
    date: Option<&str>,
) {
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", name)
        .env("GIT_AUTHOR_EMAIL", email)
        .env("GIT_COMMITTER_NAME", name)
        .env("GIT_COMMITTER_EMAIL", email);
    if let Some(date) = date {
        command
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date);
    }
    let output = command.args(args).output().expect("运行 git 命令");
    assert!(
        output.status.success(),
        "git 命令失败: {}\nstdout: {}\nstderr: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_expect_failure<const N: usize>(repo: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("run git command");

    assert!(
        !output.status.success(),
        "git command unexpectedly succeeded: {}\nstdout: {}\nstderr: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

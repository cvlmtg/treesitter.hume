use std::fs;

use super::*;
use crate::test_support::{file_url, git, serving_repo};

/// A repository with two commits. Returns the directory and the first commit, which is not the tip.
fn fixture() -> (tempfile::TempDir, String) {
    let dir = serving_repo();
    let root = dir.path();
    fs::create_dir_all(root.join("runtime/queries/c")).expect("queries dir");
    fs::write(
        root.join("runtime/queries/c/highlights.scm"),
        "(identifier) @variable\n",
    )
    .expect("query");
    fs::create_dir_all(root.join("src")).expect("src dir");
    fs::write(root.join("src/parser.c"), "first\n").expect("parser");
    git(root, &["add", "."]);
    git(root, &["commit", "-q", "-m", "first"]);
    let first = git(root, &["rev-parse", "HEAD"]);
    fs::write(root.join("src/parser.c"), "second\n").expect("parser");
    git(root, &["commit", "-q", "-am", "second"]);
    (dir, first)
}

#[test]
fn a_checkout_is_of_the_pinned_commit() {
    let (repo, first) = fixture();
    let dest = tempfile::tempdir().expect("dest");
    Git::without_retries()
        .checkout(&file_url(repo.path()), &first, dest.path(), None)
        .expect("checkout");
    assert_eq!(
        fs::read_to_string(dest.path().join("src/parser.c")).expect("parser"),
        "first\n"
    );
}

#[test]
fn a_sparse_checkout_holds_only_its_directory() {
    let (repo, first) = fixture();
    let dest = tempfile::tempdir().expect("dest");
    Git::without_retries()
        .checkout(
            &file_url(repo.path()),
            &first,
            dest.path(),
            Some("runtime/queries"),
        )
        .expect("checkout");
    assert!(
        dest.path()
            .join("runtime/queries/c/highlights.scm")
            .is_file()
    );
    assert!(!dest.path().join("src").exists());
}

/// Line endings stay as committed whatever the machine's git defaults, so
/// a shipped query file is the same on every platform.
#[test]
fn a_checkout_keeps_line_endings() {
    let (repo, first) = fixture();
    let dest = tempfile::tempdir().expect("dest");
    Git::without_retries()
        .checkout(&file_url(repo.path()), &first, dest.path(), None)
        .expect("checkout");
    let query = fs::read(dest.path().join("runtime/queries/c/highlights.scm")).expect("query");
    assert!(!query.contains(&b'\r'));
}

#[test]
fn an_unknown_commit_is_an_error_naming_it() {
    let (repo, _) = fixture();
    let dest = tempfile::tempdir().expect("dest");
    let missing = "0".repeat(40);
    let err = Git::without_retries()
        .checkout(&file_url(repo.path()), &missing, dest.path(), None)
        .expect_err("no such commit");
    assert!(err.to_string().contains(&missing), "{err}");
}

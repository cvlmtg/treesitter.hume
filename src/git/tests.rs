use std::fs;

use super::*;
use crate::test_support::{file_url, git, serving_repo};

/// A repository with two commits. Returns the directory and the first commit, which is not the tip.
fn fixture() -> (tempfile::TempDir, String) {
    let dir = serving_repo();
    let root = dir.path();
    fs::write(root.join("LICENSE"), "terms\n").expect("license");
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
        .checkout(&file_url(repo.path()), &first, dest.path())
        .expect("checkout");
    assert_eq!(
        fs::read_to_string(dest.path().join("src/parser.c")).expect("parser"),
        "first\n"
    );
}

/// Line endings stay as committed whatever the machine's git defaults, so
/// a shipped license file is the same on every platform.
#[test]
fn a_checkout_keeps_line_endings() {
    let (repo, first) = fixture();
    let dest = tempfile::tempdir().expect("dest");
    Git::without_retries()
        .checkout(&file_url(repo.path()), &first, dest.path())
        .expect("checkout");
    let license = fs::read(dest.path().join("LICENSE")).expect("license");
    assert!(!license.contains(&b'\r'));
}

#[test]
fn an_unknown_commit_is_an_error_naming_it() {
    let (repo, _) = fixture();
    let dest = tempfile::tempdir().expect("dest");
    let missing = "0".repeat(40);
    let err = Git::without_retries()
        .checkout(&file_url(repo.path()), &missing, dest.path())
        .expect_err("no such commit");
    assert!(err.to_string().contains(&missing), "{err}");
}

use std::fs;
use std::path::Path;
use std::process::Command;

use super::*;

/// Run git in `dir` for a fixture, panicking on failure.
fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run git");
    assert!(output.status.success(), "git {args:?}: {output:?}");
    String::from_utf8(output.stdout)
        .expect("UTF-8")
        .trim()
        .to_string()
}

/// A repository with two commits, serving any commit by SHA as GitHub
/// does. Returns the directory and the first commit, which is not the tip.
fn fixture() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().expect("fixture dir");
    let root = dir.path();
    git(root, &["init", "-q"]);
    git(root, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    git(root, &["config", "uploadpack.allowFilter", "true"]);
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

fn file_url(path: &Path) -> String {
    let path = path.to_str().expect("UTF-8 temp path").replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

fn quick() -> Git {
    Git {
        retry_delays: Vec::new(),
    }
}

#[test]
fn a_checkout_is_of_the_pinned_commit() {
    let (repo, first) = fixture();
    let dest = tempfile::tempdir().expect("dest");
    quick()
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
    quick()
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
    quick()
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
    let err = quick()
        .checkout(&file_url(repo.path()), &missing, dest.path(), None)
        .expect_err("no such commit");
    assert!(err.to_string().contains(&missing), "{err}");
}

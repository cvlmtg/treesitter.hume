//! Fixtures shared by unit tests.

use std::fs;
use std::path::Path;
use std::process::Command;

/// A grammar directory whose `src/` holds `files`.
pub(crate) fn grammar(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::create_dir(dir.path().join("src")).expect("src dir");
    for (name, body) in files {
        fs::write(dir.path().join("src").join(name), body).expect("write source");
    }
    dir
}

/// Run git in `dir` for a fixture, returning what it printed.
pub(crate) fn git(dir: &Path, args: &[&str]) -> String {
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

/// A new repository that serves any commit by SHA, with a blob filter, as
/// GitHub does.
pub(crate) fn serving_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("repo dir");
    git(dir.path(), &["init", "-q"]);
    git(
        dir.path(),
        &["config", "uploadpack.allowAnySHA1InWant", "true"],
    );
    git(dir.path(), &["config", "uploadpack.allowFilter", "true"]);
    dir
}

/// A `file://` URL for `path`, which git needs to honour `--depth`.
pub(crate) fn file_url(path: &Path) -> String {
    let path = path.to_str().expect("UTF-8 temp path").replace('\\', "/");
    if path.starts_with('/') {
        format!("file://{path}")
    } else {
        format!("file:///{path}")
    }
}

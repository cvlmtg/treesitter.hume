//! Fixtures shared by unit tests.

use std::fs;

/// A grammar directory whose `src/` holds `files`.
pub(crate) fn grammar(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    fs::create_dir(dir.path().join("src")).expect("src dir");
    for (name, body) in files {
        fs::write(dir.path().join("src").join(name), body).expect("write source");
    }
    dir
}

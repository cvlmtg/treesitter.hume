use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // `cc` reads the target triple from cargo's build-script environment,
    // which a running binary does not have.
    let target = std::env::var("TARGET").expect("cargo sets TARGET for build scripts");
    println!("cargo:rustc-env=HUME_GRAMMAR_BUILD_TARGET={target}");

    let root =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let languages = root.join("languages");
    println!("cargo:rerun-if-changed={}", languages.display());
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    fs::write(out.join("languages.rs"), folders(&languages)).expect("write languages.rs");
}

/// A Rust array expression of `languages::Folder`s, one per folder of
/// `languages`, in name order, each file brought in with `include_str!`.
fn folders(languages: &Path) -> String {
    let mut code = String::from("[\n");
    for folder in sorted_entries(languages) {
        assert!(folder.is_dir(), "{}: not a folder", folder.display());
        let mut queries = Vec::new();
        for file in sorted_entries(&folder) {
            match file_name(&file).as_str() {
                "source.toml" | "language.toml" => {}
                query if query.ends_with(".scm") => queries.push(file),
                other => panic!("{}: unexpected file {other:?}", folder.display()),
            }
        }
        let source = folder.join("source.toml");
        assert!(source.is_file(), "{}: no source.toml", folder.display());
        writeln!(
            code,
            "    Folder {{ name: {:?}, source: include_str!({:?}), queries: &[",
            file_name(&folder),
            path_str(&source)
        )
        .expect("write to a String");
        for query in &queries {
            writeln!(
                code,
                "        ({:?}, include_str!({:?})),",
                file_name(query),
                path_str(query)
            )
            .expect("write to a String");
        }
        code.push_str("    ] },\n");
    }
    code.push(']');
    code
}

fn sorted_entries(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|entry| entry.expect("directory entry").path())
        .collect();
    entries.sort();
    entries
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_else(|| panic!("{}: not a UTF-8 name", path.display()))
        .to_string()
}

fn path_str(path: &Path) -> &str {
    path.to_str()
        .unwrap_or_else(|| panic!("{}: not a UTF-8 path", path.display()))
}

use std::fs;

use super::*;

const REV: &str = "0123456789abcdef0123456789abcdef01234567";

fn pin(subpath: &str) -> Pin {
    Pin {
        name: "probe".to_string(),
        url: "https://example.com/tree-sitter-probe".to_string(),
        rev: REV.to_string(),
        subpath: subpath.to_string(),
        symbol: "tree_sitter_probe".to_string(),
    }
}

/// A checkout holding `files`, given as `(repository path, contents)`.
fn checkout(files: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("checkout");
    for (path, contents) in files {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
        fs::write(path, contents).expect("file");
    }
    dir
}

fn paths(files: &[(String, Vec<u8>)]) -> Vec<&str> {
    files.iter().map(|(path, _)| path.as_str()).collect()
}

fn text<'a>(files: &'a [(String, Vec<u8>)], path: &str) -> &'a str {
    let (_, contents) = files
        .iter()
        .find(|(name, _)| name == path)
        .unwrap_or_else(|| panic!("{path} in {:?}", paths(files)));
    std::str::from_utf8(contents).expect("UTF-8")
}

#[test]
fn root_license_files_are_archived_beside_a_note_on_the_source() {
    let dir = checkout(&[
        ("LICENSE", "MIT terms\n"),
        ("NOTICE.txt", "attribution\n"),
        ("README.md", "readme\n"),
        ("src/parser.c", "int x;\n"),
    ]);

    let files = grammar(&pin(""), dir.path()).expect("licensed");

    assert_eq!(
        paths(&files),
        ["licenses/LICENSE", "licenses/NOTICE.txt", "licenses/SOURCE"]
    );
    assert_eq!(text(&files, "licenses/LICENSE"), "MIT terms\n");
    let source = text(&files, "licenses/SOURCE");
    assert!(
        source.contains("https://example.com/tree-sitter-probe"),
        "{source}"
    );
    assert!(source.contains(REV), "{source}");
}

#[test]
fn a_subpath_adds_its_own_license_files_to_the_roots() {
    let dir = checkout(&[
        ("LICENSE-MIT", "root terms\n"),
        ("typescript/LICENSE", "typescript terms\n"),
        ("tsx/LICENSE", "tsx terms\n"),
    ]);

    let files = grammar(&pin("typescript"), dir.path()).expect("licensed");

    assert_eq!(
        paths(&files),
        [
            "licenses/LICENSE-MIT",
            "licenses/SOURCE",
            "licenses/typescript/LICENSE"
        ]
    );
    assert_eq!(
        text(&files, "licenses/typescript/LICENSE"),
        "typescript terms\n"
    );
    assert!(text(&files, "licenses/SOURCE").contains("typescript"));
}

#[test]
fn a_license_in_the_subpath_alone_is_enough() {
    let dir = checkout(&[("crates/tree-sitter-ebnf/LICENSE", "terms\n")]);

    let files = grammar(&pin("crates/tree-sitter-ebnf"), dir.path()).expect("licensed");

    assert_eq!(
        paths(&files),
        [
            "licenses/SOURCE",
            "licenses/crates/tree-sitter-ebnf/LICENSE"
        ]
    );
}

#[test]
fn a_grammar_without_a_license_file_is_an_error() {
    let dir = checkout(&[("README.md", "readme\n"), ("LICENSES/MIT.txt", "terms\n")]);

    let err = grammar(&pin(""), dir.path()).expect_err("unlicensed");

    assert_eq!(
        err.to_string(),
        format!("no license file in the repository root at {REV}")
    );
}

#[test]
fn license_files_are_named_for_a_license_copying_or_notice() {
    for name in [
        "LICENSE",
        "license",
        "License.txt",
        "LICENSE.md",
        "LICENSE-APACHE",
        "LICENCE",
        "COPYING",
        "COPYING.txt",
        "NOTICE",
        "UNLICENSE",
    ] {
        assert!(is_license_name(name), "{name}");
    }
    for name in ["README.md", "LICENSES", "licensee.js", "NOTICES", "SOURCE"] {
        assert!(!is_license_name(name), "{name}");
    }
}

#[test]
fn the_queries_notice_names_helix_its_commit_and_the_mpl() {
    let notice = queries_notice("probe", "https://github.com/helix-editor/helix", REV);

    for part in [
        "runtime/queries/probe/",
        "https://github.com/helix-editor/helix",
        REV,
        "Mozilla Public License 2.0",
        "https://mozilla.org/MPL/2.0/",
    ] {
        assert!(notice.contains(part), "{part} in {notice}");
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::compile::build;

/// The generated grammar fixture, whose `parser.c` declares ABI 15.
fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe")
}

/// Build the grammar in `grammar_dir` into a library in a fresh directory.
fn library(grammar_dir: &Path) -> (tempfile::TempDir, PathBuf) {
    let out = tempfile::tempdir().expect("out dir");
    let library = out
        .path()
        .join(format!("probe.{}", std::env::consts::DLL_EXTENSION));
    build(grammar_dir, &library).expect("build");
    (out, library)
}

#[test]
fn the_abi_is_read_from_the_built_library() {
    let (_out, library) = library(&fixture());
    assert_eq!(abi(&library, "tree_sitter_probe").expect("probe"), 15);
}

#[test]
fn a_missing_symbol_is_an_error_naming_it() {
    let (_out, library) = library(&fixture());
    let err = abi(&library, "tree_sitter_other").expect_err("no such symbol");
    assert!(err.to_string().contains("tree_sitter_other"), "{err}");
}

/// A copy of the fixture whose `parser.c` declares ABI 12, below every
/// version tree-sitter 0.27 loads.
#[test]
fn an_unsupported_abi_is_an_error() {
    let grammar = tempfile::tempdir().expect("grammar dir");
    let src = grammar.path().join("src");
    fs::create_dir_all(src.join("tree_sitter")).expect("src dir");
    for header in ["alloc.h", "array.h", "parser.h"] {
        fs::copy(
            fixture().join("src/tree_sitter").join(header),
            src.join("tree_sitter").join(header),
        )
        .expect("header");
    }
    let parser = fs::read_to_string(fixture().join("src/parser.c")).expect("parser");
    let parser = parser.replace("#define LANGUAGE_VERSION 15", "#define LANGUAGE_VERSION 12");
    fs::write(src.join("parser.c"), parser).expect("parser");
    let (_out, library) = library(grammar.path());
    let err = abi(&library, "tree_sitter_probe").expect_err("ABI 12");
    assert!(err.to_string().contains("ABI 12"), "{err}");
}

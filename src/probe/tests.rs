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
    assert_eq!(
        check(&library, "tree_sitter_probe", &[]).expect("probe"),
        15
    );
}

#[test]
fn a_missing_symbol_is_an_error_naming_it() {
    let (_out, library) = library(&fixture());
    let err = check(&library, "tree_sitter_other", &[]).expect_err("no such symbol");
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
    let err = check(&library, "tree_sitter_probe", &[]).expect_err("ABI 12");
    assert!(err.to_string().contains("ABI 12"), "{err}");
}

fn queries(files: &[(&str, &str)]) -> Vec<(String, String)> {
    files
        .iter()
        .map(|(file, text)| (file.to_string(), text.to_string()))
        .collect()
}

#[test]
fn queries_that_match_the_grammar_compile() {
    let (_out, library) = library(&fixture());
    let queries = queries(&[
        ("highlights.scm", "(word) @variable\n"),
        ("context.scm", "(source) @context\n"),
    ]);
    assert_eq!(
        check(&library, "tree_sitter_probe", &queries).expect("probe"),
        15
    );
}

#[test]
fn a_query_naming_a_node_the_grammar_lacks_is_an_error_naming_its_file() {
    let (_out, library) = library(&fixture());
    let queries = queries(&[
        ("highlights.scm", "(word) @variable\n"),
        ("indents.scm", "(word) @indent\n(function_item) @indent\n"),
    ]);
    let err = check(&library, "tree_sitter_probe", &queries).expect_err("unknown node");
    let message = err.to_string();
    assert!(message.starts_with("queries/indents.scm: "), "{message}");
    assert!(message.contains("function_item"), "{message}");
}

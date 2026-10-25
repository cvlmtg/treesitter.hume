use std::fs;

use super::*;
use crate::test_support::grammar;

const PARSER: &str = "int tree_sitter_probe_scanner(void);\n\
     int tree_sitter_probe(void) { return tree_sitter_probe_scanner(); }\n";
const CPP_SCANNER: &str = "#include <vector>\n\
     extern \"C\" int tree_sitter_probe_scanner() { std::vector<int> v{1, 2}; return (int)v.size(); }\n";

#[test]
fn build_links_a_cpp_scanner_and_leaves_only_the_library() {
    let dir = grammar(&[("parser.c", PARSER), ("scanner.cc", CPP_SCANNER)]);
    let out_dir = tempfile::tempdir().expect("out dir");
    let out = out_dir.path().join("probe.lib");
    build(dir.path(), &out).expect("build");
    let left: Vec<_> = fs::read_dir(out_dir.path())
        .expect("read out dir")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(left, vec![out.file_name().expect("file name").to_owned()]);
    assert!(fs::metadata(&out).expect("library").len() > 0);
}

/// A first install has no grammars directory yet.
#[test]
fn build_creates_the_output_directory() {
    let dir = grammar(&[("parser.c", "int tree_sitter_probe(void) { return 0; }\n")]);
    let root = tempfile::tempdir().expect("root");
    let out = root.path().join("grammars").join("probe.lib");
    build(dir.path(), &out).expect("build");
    assert!(fs::metadata(&out).expect("library").len() > 0);
}

#[test]
fn failed_build_keeps_the_previous_library() {
    let dir = grammar(&[("parser.c", "this is not C\n")]);
    let out_dir = tempfile::tempdir().expect("out dir");
    let out = out_dir.path().join("probe.lib");
    fs::write(&out, "previous").expect("previous library");
    let err = build(dir.path(), &out).expect_err("parser.c does not compile");
    assert!(matches!(err, CompileError::Failed { .. }));
    assert_eq!(fs::read_to_string(&out).expect("library"), "previous");
    assert_eq!(
        fs::read_dir(out_dir.path()).expect("read out dir").count(),
        1
    );
}

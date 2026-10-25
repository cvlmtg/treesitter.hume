use std::fs;
use std::process::Command;

/// `zig cc` and `zig c++` named in `CC`/`CXX` build a grammar whose scanner
/// is C++: zig runs without `cc`, which would pass it a `--target` zig
/// rejects, and the scanner object links into the shared library.
#[test]
#[ignore = "needs zig on PATH; CI runs it with --ignored"]
fn zig_builds_a_grammar_with_a_cpp_scanner() {
    let grammar = tempfile::tempdir().expect("grammar dir");
    let src = grammar.path().join("src");
    fs::create_dir(&src).expect("src dir");
    fs::write(
        src.join("parser.c"),
        "int tree_sitter_probe_scanner(void);\n\
         int tree_sitter_probe(void) { return tree_sitter_probe_scanner(); }\n",
    )
    .expect("write parser");
    fs::write(
        src.join("scanner.cc"),
        "#include <vector>\n\
         extern \"C\" int tree_sitter_probe_scanner() { std::vector<int> v{1, 2}; return (int)v.size(); }\n",
    )
    .expect("write scanner");
    let out_dir = tempfile::tempdir().expect("out dir");
    let out = out_dir.path().join("probe.lib");

    let status = Command::new(env!("CARGO_BIN_EXE_hume-grammar-build"))
        .env("CC", "zig cc")
        .env("CXX", "zig c++")
        .arg("-o")
        .arg(&out)
        .arg(grammar.path())
        .status()
        .expect("run hume-grammar-build");

    assert!(status.success(), "build with zig: {status}");
    assert!(fs::metadata(&out).expect("library").len() > 0);
}

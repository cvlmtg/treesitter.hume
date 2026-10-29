use std::fs;
use std::path::PathBuf;
use std::process::Command;

use tempfile::TempDir;

/// Builds a grammar whose C++ scanner is `scanner`, with `zig cc` and
/// `zig c++` named in `CC`/`CXX`. The scanner defines
/// `tree_sitter_probe_scanner`, which the parser calls.
fn build_with_zig(scanner: &str) -> (TempDir, PathBuf) {
    let grammar = tempfile::tempdir().expect("grammar dir");
    let src = grammar.path().join("src");
    fs::create_dir(&src).expect("src dir");
    fs::write(
        src.join("parser.c"),
        "int tree_sitter_probe_scanner(void);\n\
         int tree_sitter_probe(void) { return tree_sitter_probe_scanner(); }\n",
    )
    .expect("write parser");
    fs::write(src.join("scanner.cc"), scanner).expect("write scanner");
    let out_dir = tempfile::tempdir().expect("out dir");
    let out = out_dir.path().join("probe.lib");

    let status = Command::new(env!("CARGO_BIN_EXE_hume-grammar-build"))
        .env("CC", "zig cc")
        .env("CXX", "zig c++")
        .arg("compile")
        .arg(grammar.path())
        .arg(&out)
        .status()
        .expect("run hume-grammar-build");

    assert!(status.success(), "build with zig: {status}");
    (out_dir, out)
}

/// `zig cc` and `zig c++` named in `CC`/`CXX` build a grammar whose scanner
/// is C++: zig runs without `cc`, which would pass it a `--target` zig
/// rejects, and the scanner object links into the shared library.
#[test]
#[ignore = "needs zig on PATH; CI runs it with --ignored"]
fn zig_builds_a_grammar_with_a_cpp_scanner() {
    let (_out_dir, out) = build_with_zig(
        "#include <vector>\n\
         extern \"C\" int tree_sitter_probe_scanner() { std::vector<int> v{1, 2}; return (int)v.size(); }\n",
    );
    assert!(fs::metadata(&out).expect("library").len() > 0);
}

/// A process that loads and unloads a zig-built grammar holding a C++
/// static with a destructor exits cleanly: the destructor runs at unload,
/// while its code is still mapped, instead of at exit.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[test]
#[ignore = "needs zig on PATH; CI runs it with --ignored"]
fn a_zig_built_grammar_can_be_unloaded_before_exit() {
    let (out_dir, library) = build_with_zig(
        "#include <string>\n\
         static std::string text(100, 'x');\n\
         extern \"C\" int tree_sitter_probe_scanner() { return (int)text.size(); }\n",
    );
    let loader_source = out_dir.path().join("loader.c");
    fs::write(
        &loader_source,
        "#include <dlfcn.h>\n\
         #include <stdio.h>\n\
         int main(int argc, char **argv) {\n\
             void *handle = dlopen(argv[1], RTLD_NOW);\n\
             if (!handle) { fprintf(stderr, \"%s\\n\", dlerror()); return 2; }\n\
             dlclose(handle);\n\
             return 0;\n\
         }\n",
    )
    .expect("write loader");
    let loader = out_dir.path().join("loader");
    let status = Command::new("zig")
        .arg("cc")
        .arg(&loader_source)
        .arg("-o")
        .arg(&loader)
        .status()
        .expect("run zig cc");
    assert!(status.success(), "build the loader: {status}");

    let status = Command::new(&loader)
        .arg(&library)
        .status()
        .expect("run the loader");

    assert!(status.success(), "load, unload and exit: {status}");
}

use super::*;
use crate::test_support::grammar;

#[test]
fn sources_prefer_a_c_scanner_over_a_cpp_one() {
    let dir = grammar(&[("parser.c", ""), ("scanner.c", ""), ("scanner.cc", "")]);
    let sources = Sources::find(dir.path()).expect("sources");
    assert!(matches!(sources.scanner, Some(Scanner::C(path)) if path.ends_with("scanner.c")));
}

#[test]
fn sources_find_a_cpp_scanner() {
    let dir = grammar(&[("parser.c", ""), ("scanner.cc", "")]);
    let sources = Sources::find(dir.path()).expect("sources");
    assert!(matches!(sources.scanner, Some(Scanner::Cpp(path)) if path.ends_with("scanner.cc")));
}

#[test]
fn sources_find_a_cpp_scanner_with_the_cpp_extension() {
    let dir = grammar(&[("parser.c", ""), ("scanner.cpp", "")]);
    let sources = Sources::find(dir.path()).expect("sources");
    assert!(matches!(sources.scanner, Some(Scanner::Cpp(path)) if path.ends_with("scanner.cpp")));
}

#[test]
fn sources_without_a_scanner_have_none() {
    let dir = grammar(&[("parser.c", "")]);
    let sources = Sources::find(dir.path()).expect("sources");
    assert!(sources.scanner.is_none());
}

#[test]
fn sources_without_a_parser_are_refused() {
    let dir = grammar(&[("scanner.c", "")]);
    let err = Sources::find(dir.path()).expect_err("no parser.c");
    assert!(matches!(err, CompileError::NoParser(path) if path.ends_with("parser.c")));
}

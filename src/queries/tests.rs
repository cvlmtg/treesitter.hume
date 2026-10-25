use std::fs;

use super::*;

/// A reader over `(language, text)` pairs, all for one query file.
fn reader<'a>(files: &'a [(&str, &str)]) -> impl Fn(&str) -> Option<String> + 'a {
    move |language| {
        files
            .iter()
            .find(|(name, _)| *name == language)
            .map(|(_, text)| text.to_string())
    }
}

fn resolved(language: &str, files: &[(&str, &str)]) -> Resolved {
    resolve(language, reader(files)).expect("resolves")
}

#[test]
fn a_query_without_a_directive_is_unchanged() {
    let query = resolved("c", &[("c", "(identifier) @variable\n")]);
    assert_eq!(query.text, "(identifier) @variable\n");
    assert!(query.missing.is_empty());
}

/// The directive and the whitespace after it give way to each parent,
/// framed by newlines.
#[test]
fn a_parent_replaces_the_directive_in_place() {
    let query = resolved(
        "cpp",
        &[("cpp", "; inherits: c\n(this) @keyword\n"), ("c", "(c)\n")],
    );
    assert_eq!(query.text, "\n(c)\n\n(this) @keyword\n");
}

#[test]
fn parents_are_inserted_in_the_order_listed() {
    let query = resolved(
        "tsx",
        &[
            ("tsx", "; inherits: a,b\n(tsx)\n"),
            ("a", "(a)"),
            ("b", "(b)"),
        ],
    );
    assert_eq!(query.text, "\n(a)\n\n(b)\n(tsx)\n");
}

#[test]
fn every_directive_is_replaced() {
    let query = resolved(
        "x",
        &[
            ("x", "(head)\n; inherits: a\n(mid)\n; inherits: b\n"),
            ("a", "(a)"),
            ("b", "(b)"),
        ],
    );
    assert_eq!(query.text, "(head)\n\n(a)\n(mid)\n\n(b)\n");
}

#[test]
fn the_directive_may_double_its_semicolon_and_drop_its_colon() {
    let query = resolved("x", &[("x", ";; inherits a\n(x)\n"), ("a", "(a)")]);
    assert_eq!(query.text, "\n(a)\n(x)\n");
}

/// Parent names stop at a space, so `a, b` inherits `a` and an empty name
/// and leaves `b` in the text, as Helix does.
#[test]
fn a_space_after_a_comma_ends_the_parent_list() {
    let query = resolved("x", &[("x", "; inherits: a, b\n"), ("a", "(a)")]);
    assert_eq!(query.text, "\n(a)\n\n\nb\n");
    assert_eq!(query.missing, [""]);
}

#[test]
fn parents_resolve_their_own_parents() {
    let query = resolved(
        "tsx",
        &[
            ("tsx", "; inherits: typescript\n(tsx)\n"),
            ("typescript", "; inherits: ecma\n(ts)\n"),
            ("ecma", "(ecma)\n"),
        ],
    );
    assert_eq!(query.text, "\n\n(ecma)\n\n(ts)\n\n(tsx)\n");
}

#[test]
fn a_missing_parent_is_empty_and_reported() {
    let query = resolved("x", &[("x", "; inherits: gone\n(x)\n")]);
    assert_eq!(query.text, "\n\n(x)\n");
    assert_eq!(query.missing, ["gone"]);
}

#[test]
fn a_cycle_is_an_error_naming_its_chain() {
    let err = resolve(
        "a",
        reader(&[("a", "; inherits: b\n"), ("b", "; inherits: a\n")]),
    )
    .expect_err("a inherits itself through b");
    assert_eq!(err.to_string(), "query inheritance cycle: a -> b -> a");
}

#[test]
fn a_language_whose_file_is_missing_is_an_error() {
    let err = resolve("x", reader(&[])).expect_err("no file for x");
    assert_eq!(err.to_string(), "no query file for x");
}

#[test]
fn query_files_lists_the_scm_files_in_name_order() {
    let dir = tempfile::tempdir().expect("dir");
    for name in ["textobjects.scm", "highlights.scm", "README.md"] {
        fs::write(dir.path().join(name), "").expect("file");
    }
    fs::create_dir(dir.path().join("nested.scm")).expect("dir");
    assert_eq!(
        query_files(dir.path()).expect("list"),
        ["highlights.scm", "textobjects.scm"]
    );
}

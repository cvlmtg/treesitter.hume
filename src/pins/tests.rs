use super::*;

const REV: &str = "0123456789abcdef0123456789abcdef01234567";

fn language(name: &str, rev: &str, symbol: &str) -> String {
    format!(
        "[[language]]\nname = \"{name}\"\nurl = \"https://example.com/{name}\"\nrev = \"{rev}\"\nsubpath = \"\"\nsymbol = \"{symbol}\"\n"
    )
}

fn grammars(languages: &[String]) -> String {
    format!("helix-rev = \"{REV}\"\n{}", languages.concat())
}

fn two_languages() -> String {
    grammars(&[
        language("javascript", REV, "tree_sitter_javascript"),
        language("markdown.inline", REV, "tree_sitter_markdown_inline"),
    ])
}

fn names<'a>(pins: impl IntoIterator<Item = &'a Pin>) -> Vec<&'a str> {
    pins.into_iter().map(|pin| pin.name.as_str()).collect()
}

#[test]
fn the_embedded_pins_parse() {
    let pins = Pins::embedded().expect("embedded pins");
    assert_eq!(pins.helix_rev().len(), 40);
    let rust = pins.get("rust").expect("rust is pinned");
    assert_eq!(rust.symbol, "tree_sitter_rust");
    assert_eq!(rust.subpath, "");
    let inline = pins
        .get("markdown.inline")
        .expect("markdown.inline is pinned");
    assert_eq!(inline.subpath, "tree-sitter-markdown-inline");
}

#[test]
fn a_release_leaves_out_what_its_platform_excludes() {
    let exclusions =
        "[[exclude]]\nname = \"javascript\"\nplatforms = [\"windows-x86_64\"]\nreason = \"r\"\n";
    let pins = Pins::parse(&two_languages(), exclusions).expect("pins");
    assert_eq!(names(pins.release("windows-x86_64")), ["markdown.inline"]);
    assert_eq!(
        names(pins.release("linux-x86_64")),
        ["javascript", "markdown.inline"]
    );
}

#[test]
fn an_exclusion_without_platforms_applies_to_every_platform() {
    let exclusions = "[[exclude]]\nname = \"javascript\"\nreason = \"r\"\n";
    let pins = Pins::parse(&two_languages(), exclusions).expect("pins");
    assert_eq!(names(pins.release("linux-x86_64")), ["markdown.inline"]);
}

#[test]
fn an_empty_exclusion_list_excludes_nothing() {
    let pins = Pins::parse(&two_languages(), "").expect("pins");
    assert_eq!(pins.release("linux-x86_64").len(), 2);
}

fn refused(grammars: &str, exclusions: &str) -> String {
    Pins::parse(grammars, exclusions)
        .err()
        .expect("invalid pins are refused")
        .to_string()
}

#[test]
fn an_abbreviated_rev_is_refused() {
    let message = refused(&grammars(&[language("c", "0123456", "tree_sitter_c")]), "");
    assert!(message.contains("0123456"), "{message}");
}

#[test]
fn an_abbreviated_helix_rev_is_refused() {
    let message = refused("helix-rev = \"ba40e547426b\"\n", "");
    assert!(message.contains("ba40e547426b"), "{message}");
}

#[test]
fn a_name_that_cannot_name_an_archive_is_refused() {
    for name in ["Rust", ".hidden", "a/b", "", &"x".repeat(94)] {
        let message = refused(&grammars(&[language(name, REV, "tree_sitter_x")]), "");
        assert!(message.contains("name"), "{name}: {message}");
    }
}

#[test]
fn a_symbol_that_is_not_a_c_identifier_is_refused() {
    let message = refused(&grammars(&[language("c", REV, "tree-sitter-c")]), "");
    assert!(message.contains("tree-sitter-c"), "{message}");
}

#[test]
fn a_duplicate_language_is_refused() {
    let c = language("c", REV, "tree_sitter_c");
    let message = refused(&grammars(&[c.clone(), c]), "");
    assert!(message.contains("\"c\""), "{message}");
}

#[test]
fn an_exclusion_of_an_unpinned_language_is_refused() {
    let exclusions = "[[exclude]]\nname = \"cobol\"\nreason = \"r\"\n";
    let message = refused(&two_languages(), exclusions);
    assert!(message.contains("cobol"), "{message}");
}

#[test]
fn an_exclusion_without_a_reason_is_refused() {
    let exclusions = "[[exclude]]\nname = \"javascript\"\nreason = \" \"\n";
    let message = refused(&two_languages(), exclusions);
    assert!(message.contains("javascript"), "{message}");
}

#[test]
fn an_unknown_key_is_refused() {
    let exclusions = "[[exclude]]\nname = \"javascript\"\nreason = \"r\"\nplatform = \"x\"\n";
    let message = refused(&two_languages(), exclusions);
    assert!(message.contains("platform"), "{message}");
}

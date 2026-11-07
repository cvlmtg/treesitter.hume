use super::*;

const REV: &str = "0123456789abcdef0123456789abcdef01234567";
const HIGHLIGHTS: &[(&str, &str)] = &[("highlights.scm", "(word) @variable\n")];

const JAVASCRIPT: &str = "[upstream]
helix = \"0123456789abcdef0123456789abcdef01234567\"

[grammar]
url = \"https://example.com/javascript\"
rev = \"0123456789abcdef0123456789abcdef01234567\"
subpath = \"\"
symbol = \"tree_sitter_javascript\"
";

const MARKDOWN_INLINE: &str = "[upstream]
helix = \"0123456789abcdef0123456789abcdef01234567\"

[grammar]
url = \"https://example.com/markdown\"
rev = \"0123456789abcdef0123456789abcdef01234567\"
subpath = \"tree-sitter-markdown-inline\"
symbol = \"tree_sitter_markdown_inline\"
";

fn upstream(rev: &str) -> String {
    format!("[upstream]\nhelix = \"{rev}\"\n")
}

fn pinned(name: &str, rev: &str, symbol: &str) -> String {
    format!(
        "{}\n[grammar]\nurl = \"https://example.com/{name}\"\nrev = \"{rev}\"\nsubpath = \"\"\nsymbol = \"{symbol}\"\n",
        upstream(REV)
    )
}

fn folder<'a>(name: &'a str, source: &'a str, queries: &'a [(&'a str, &'a str)]) -> Folder<'a> {
    Folder {
        name,
        source,
        queries,
    }
}

fn names<'a>(pins: impl IntoIterator<Item = &'a Pin>) -> Vec<&'a str> {
    pins.into_iter().map(|pin| pin.name.as_str()).collect()
}

fn two_languages(exclusions: &str) -> Result<Languages<'static>, LanguagesError> {
    Languages::parse(
        vec![
            folder("javascript", JAVASCRIPT, HIGHLIGHTS),
            folder("markdown.inline", MARKDOWN_INLINE, HIGHLIGHTS),
        ],
        exclusions,
    )
}

fn refused(folders: Vec<Folder>, exclusions: &str) -> String {
    Languages::parse(folders, exclusions)
        .err()
        .expect("invalid languages are refused")
        .to_string()
}

#[test]
fn the_embedded_languages_parse() {
    let languages = Languages::embedded().expect("embedded languages");
    let rust = languages.get("rust").expect("rust is pinned");
    assert_eq!(rust.symbol, "tree_sitter_rust");
    assert_eq!(rust.subpath, "");
    assert!(rust.upstream.context.is_some());
    assert!(languages.query("rust", "highlights.scm").is_some());
    let inline = languages
        .get("markdown.inline")
        .expect("markdown.inline is pinned");
    assert_eq!(inline.subpath, "tree-sitter-markdown-inline");
    assert!(languages.get("ecma").is_none(), "ecma holds queries only");
    assert!(languages.query("ecma", "highlights.scm").is_some());
}

#[test]
fn a_folder_lists_its_query_files_in_name_order() {
    let source = pinned("probe", REV, "tree_sitter_probe");
    let queries = [("textobjects.scm", "a"), ("highlights.scm", "b")];
    let languages =
        Languages::parse(vec![folder("probe", &source, &queries)], "").expect("languages");
    assert_eq!(
        languages.query_files("probe"),
        ["highlights.scm", "textobjects.scm"]
    );
    assert_eq!(languages.query("probe", "textobjects.scm"), Some("a"));
    assert_eq!(languages.query("probe", "indents.scm"), None);
}

#[test]
fn a_release_leaves_out_what_its_platform_excludes() {
    let exclusions =
        "[[exclude]]\nname = \"javascript\"\nplatforms = [\"windows-x86_64\"]\nreason = \"r\"\n";
    let languages = two_languages(exclusions).expect("languages");
    assert_eq!(
        names(languages.release("windows-x86_64")),
        ["markdown.inline"]
    );
    assert_eq!(
        names(languages.release("linux-x86_64")),
        ["javascript", "markdown.inline"]
    );
}

#[test]
fn an_exclusion_without_platforms_applies_to_every_platform() {
    let exclusions = "[[exclude]]\nname = \"javascript\"\nreason = \"r\"\n";
    let languages = two_languages(exclusions).expect("languages");
    assert_eq!(
        names(languages.release("linux-x86_64")),
        ["markdown.inline"]
    );
}

#[test]
fn an_empty_exclusion_list_excludes_nothing() {
    let languages = two_languages("").expect("languages");
    assert_eq!(languages.release("linux-x86_64").len(), 2);
}

#[test]
fn an_abbreviated_grammar_rev_is_refused() {
    let source = pinned("c", "0123456", "tree_sitter_c");
    let message = refused(vec![folder("c", &source, HIGHLIGHTS)], "");
    assert!(message.contains("0123456"), "{message}");
}

#[test]
fn an_abbreviated_upstream_rev_is_refused() {
    let source = upstream("ba40e547426b");
    let message = refused(vec![folder("ecma", &source, HIGHLIGHTS)], "");
    assert!(message.contains("ba40e547426b"), "{message}");
}

#[test]
fn a_name_that_cannot_name_an_archive_is_refused() {
    for name in ["Rust", ".hidden", "a/b", "", &"x".repeat(94)] {
        let source = pinned(name, REV, "tree_sitter_x");
        let message = refused(vec![folder(name, &source, HIGHLIGHTS)], "");
        assert!(message.contains("name"), "{name}: {message}");
    }
}

#[test]
fn a_symbol_that_is_not_a_c_identifier_is_refused() {
    let source = pinned("c", REV, "tree-sitter-c");
    let message = refused(vec![folder("c", &source, HIGHLIGHTS)], "");
    assert!(message.contains("tree-sitter-c"), "{message}");
}

#[test]
fn a_duplicate_folder_is_refused() {
    let source = pinned("c", REV, "tree_sitter_c");
    let message = refused(
        vec![
            folder("c", &source, HIGHLIGHTS),
            folder("c", &source, HIGHLIGHTS),
        ],
        "",
    );
    assert!(message.contains("\"c\""), "{message}");
}

#[test]
fn a_pinned_language_without_highlights_is_refused() {
    let source = pinned("c", REV, "tree_sitter_c");
    let message = refused(vec![folder("c", &source, &[("indents.scm", "")])], "");
    assert!(message.contains("highlights.scm"), "{message}");
}

#[test]
fn context_queries_without_a_context_rev_are_refused() {
    let source = pinned("c", REV, "tree_sitter_c");
    let queries = [("context.scm", ""), ("highlights.scm", "")];
    let message = refused(vec![folder("c", &source, &queries)], "");
    assert!(message.contains("context"), "{message}");
}

#[test]
fn a_context_rev_without_context_queries_is_refused() {
    let source = format!("[upstream]\nhelix = \"{REV}\"\ncontext = \"{REV}\"\n");
    let message = refused(vec![folder("ecma", &source, HIGHLIGHTS)], "");
    assert!(message.contains("context"), "{message}");
}

#[test]
fn an_exclusion_of_an_unpinned_language_is_refused() {
    let message = two_languages("[[exclude]]\nname = \"cobol\"\nreason = \"r\"\n")
        .err()
        .expect("refused")
        .to_string();
    assert!(message.contains("cobol"), "{message}");
}

#[test]
fn an_exclusion_without_a_reason_is_refused() {
    let message = two_languages("[[exclude]]\nname = \"javascript\"\nreason = \" \"\n")
        .err()
        .expect("refused")
        .to_string();
    assert!(message.contains("javascript"), "{message}");
}

#[test]
fn an_unknown_key_is_refused() {
    let source = format!(
        "{}\n[grammar]\nurl = \"u\"\nbranch = \"x\"\n",
        upstream(REV)
    );
    let message = refused(vec![folder("c", &source, HIGHLIGHTS)], "");
    assert!(message.contains("c/source.toml"), "{message}");
    let exclusions = "[[exclude]]\nname = \"javascript\"\nreason = \"r\"\nplatform = \"x\"\n";
    let message = two_languages(exclusions)
        .err()
        .expect("refused")
        .to_string();
    assert!(message.contains("platform"), "{message}");
}

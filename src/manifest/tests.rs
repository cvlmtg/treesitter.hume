use super::*;

fn manifest() -> Manifest<'static> {
    Manifest {
        name: "markdown.inline",
        url: "https://github.com/tree-sitter-grammars/tree-sitter-markdown",
        rev: "f969cd3ae3f9fbd4e43205431d0ae286014c05b5",
        subpath: "tree-sitter-markdown-inline",
        symbol: "tree_sitter_markdown_inline",
        abi: 14,
        platform: "linux-x86_64",
        tag: "v0.1.0",
        helix_rev: "ba40e547426b0f9896c8bdc699a4ab11f2b37dbc",
    }
}

#[test]
fn the_manifest_is_an_alist_in_field_order() {
    assert_eq!(
        manifest().to_scheme(),
        "((name . \"markdown.inline\")\n \
         (url . \"https://github.com/tree-sitter-grammars/tree-sitter-markdown\")\n \
         (rev . \"f969cd3ae3f9fbd4e43205431d0ae286014c05b5\")\n \
         (subpath . \"tree-sitter-markdown-inline\")\n \
         (symbol . \"tree_sitter_markdown_inline\")\n \
         (abi . 14)\n \
         (platform . \"linux-x86_64\")\n \
         (tag . \"v0.1.0\")\n \
         (helix-rev . \"ba40e547426b0f9896c8bdc699a4ab11f2b37dbc\"))\n"
    );
}

#[test]
fn quotes_and_backslashes_are_escaped() {
    let manifest = Manifest {
        url: r#"https://example.com/a"b\c"#,
        ..manifest()
    };
    assert!(
        manifest
            .to_scheme()
            .contains(r#"(url . "https://example.com/a\"b\\c")"#)
    );
}

#[test]
fn a_string_field_reads_back_what_to_scheme_wrote() {
    let manifest = Manifest {
        url: r#"https://example.com/a"b\c"#,
        ..manifest()
    };
    let text = manifest.to_scheme();
    assert_eq!(
        read_string(&text, "name").as_deref(),
        Some("markdown.inline")
    );
    assert_eq!(
        read_string(&text, "url").as_deref(),
        Some(r#"https://example.com/a"b\c"#)
    );
    assert_eq!(
        read_string(&text, "rev").as_deref(),
        Some("f969cd3ae3f9fbd4e43205431d0ae286014c05b5")
    );
    assert_eq!(read_string(&text, "abi"), None);
    assert_eq!(read_string(&text, "missing"), None);
}

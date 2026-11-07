use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::*;
use crate::languages::{Folder, Languages};
use crate::test_support::{file_url, git, serving_repo};

/// Commit `files` into a new repository. Returns the repository and its
/// commit.
fn repo(files: &[(&str, &[u8])]) -> (tempfile::TempDir, String) {
    let dir = serving_repo();
    for (path, contents) in files {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
        fs::write(path, contents).expect("file");
    }
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-q", "-m", "fixture"]);
    let rev = git(dir.path(), &["rev-parse", "HEAD"]);
    (dir, rev)
}

/// The generated probe grammar, plus `extra` files, as a repository.
fn grammar_repo(extra: &[(&str, &[u8])]) -> (tempfile::TempDir, String) {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/probe");
    let files: Vec<(String, Vec<u8>)> = [
        "grammar.js",
        "src/parser.c",
        "src/tree_sitter/alloc.h",
        "src/tree_sitter/array.h",
        "src/tree_sitter/parser.h",
    ]
    .iter()
    .map(|path| {
        (
            path.to_string(),
            fs::read(fixture.join(path)).expect("fixture file"),
        )
    })
    .collect();
    let mut files: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(path, contents)| (path.as_str(), contents.as_slice()))
        .collect();
    files.extend_from_slice(extra);
    repo(&files)
}

const HELIX_REV: &str = "0123456789abcdef0123456789abcdef01234567";
const CONTEXT_REV: &str = "fedcba9876543210fedcba9876543210fedcba98";

/// `probe`'s queries: highlights that inherit `base`'s, and context
/// queries.
const PROBE_QUERIES: &[(&str, &str)] = &[
    ("context.scm", "(source) @context\n"),
    ("highlights.scm", "; inherits: base\n(word) @variable\n"),
    ("textobjects.scm", "(word) @word.inside\n"),
];
const STRING_QUERIES: &[(&str, &str)] = &[("highlights.scm", "(word) @string\n")];
const BASE_QUERIES: &[(&str, &str)] = &[("highlights.scm", "(source) @base\n")];
const BASE_SOURCE: &str = "[upstream]\nhelix = \"0123456789abcdef0123456789abcdef01234567\"\n";

/// The `source.toml` of a language pinned to `url` at `rev`, with context
/// queries when `context` is set.
fn source(url: &str, rev: &str, context: bool) -> String {
    let context = if context {
        format!("context = \"{CONTEXT_REV}\"\n")
    } else {
        String::new()
    };
    format!(
        "[upstream]\nhelix = \"{HELIX_REV}\"\n{context}\n[grammar]\nurl = \"{url}\"\nrev = \"{rev}\"\nsubpath = \"\"\nsymbol = \"tree_sitter_probe\"\n"
    )
}

/// The `source.toml` of every language in `pinned`, as `(name, url, rev)`.
fn sources(pinned: &[(&str, &str, &str)]) -> Vec<(String, String)> {
    pinned
        .iter()
        .map(|(name, url, rev)| (name.to_string(), source(url, rev, *name == "probe")))
        .collect()
}

/// The pinned languages `sources` describes, and the query-only `base`
/// folder.
fn languages(sources: &[(String, String)]) -> Languages<'_> {
    let mut folders = vec![Folder {
        name: "base",
        source: BASE_SOURCE,
        queries: BASE_QUERIES,
    }];
    folders.extend(sources.iter().map(|(name, source)| Folder {
        name,
        source,
        queries: if name == "probe" {
            PROBE_QUERIES
        } else {
            STRING_QUERIES
        },
    }));
    Languages::parse(folders, "").expect("languages")
}

fn run(languages: &Languages, out: &Path) -> Result<Report, BuildError> {
    let pins = ["probe", "twin", "broken"]
        .iter()
        .filter_map(|name| languages.get(name))
        .collect();
    build(
        &Request {
            languages: pins,
            queries: languages,
            out,
            tag: "v9.9.9",
            platform: "test-arch",
        },
        &Git::without_retries(),
    )
}

/// `language`'s archive in `out`.
fn member(out: &Path, language: &str) -> Vec<u8> {
    fs::read(out.join(format!("{language}.tar.gz"))).expect("archive")
}

/// The names of the files in `out`, sorted.
fn listing(out: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(out)
        .expect("out")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

/// `(path, text)` for each file in a language archive.
fn files(archive: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    tar.entries()
        .expect("entries")
        .map(|entry| {
            let mut entry = entry.expect("entry");
            let path = entry.path().expect("path").to_string_lossy().into_owned();
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).expect("contents");
            (path, contents)
        })
        .collect()
}

fn text(files: &[(String, Vec<u8>)], path: &str) -> String {
    let (_, contents) = files
        .iter()
        .find(|(name, _)| name == path)
        .unwrap_or_else(|| panic!("{path} in the archive"));
    String::from_utf8(contents.clone()).expect("UTF-8")
}

#[test]
fn a_build_packs_each_language_with_its_library_queries_and_manifest() {
    let (grammar, grammar_rev) = grammar_repo(&[("LICENSE", b"MIT terms\n")]);
    let url = file_url(grammar.path());
    let sources = sources(&[("probe", &url, &grammar_rev), ("twin", &url, &grammar_rev)]);
    let languages = languages(&sources);
    let out = tempfile::tempdir().expect("out");
    fs::write(out.path().join("c.tar.gz"), b"an earlier build").expect("earlier archive");

    let report = run(&languages, out.path()).expect("build");

    assert_eq!(report.built, ["probe", "twin"]);
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    let probe = files(&member(out.path(), "probe"));
    let library = format!("probe.{}", std::env::consts::DLL_EXTENSION);
    let paths: Vec<&str> = probe.iter().map(|(path, _)| path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "licenses/LICENSE",
            "licenses/SOURCE",
            "manifest.scm",
            library.as_str(),
            "queries/NOTICE",
            "queries/context.scm",
            "queries/highlights.scm",
            "queries/textobjects.scm",
        ]
    );
    assert_eq!(text(&probe, "licenses/LICENSE"), "MIT terms\n");
    assert!(text(&probe, "licenses/SOURCE").contains(&grammar_rev));
    let notice = text(&probe, "queries/NOTICE");
    assert!(notice.contains(HELIX_REV), "{notice}");
    assert!(notice.contains(CONTEXT_REV), "{notice}");
    assert_eq!(
        text(&probe, "queries/highlights.scm"),
        "\n(source) @base\n\n(word) @variable\n"
    );
    assert_eq!(text(&probe, "queries/context.scm"), "(source) @context\n");
    let manifest = text(&probe, "manifest.scm");
    for field in [
        "(name . \"probe\")",
        "(symbol . \"tree_sitter_probe\")",
        "(abi . 15)",
        "(platform . \"test-arch\")",
        "(tag . \"v9.9.9\")",
        &format!("(helix-rev . \"{HELIX_REV}\")"),
    ] {
        assert!(manifest.contains(field), "{field} in {manifest}");
    }
    let twin = files(&member(out.path(), "twin"));
    assert_eq!(text(&twin, "queries/highlights.scm"), "(word) @string\n");
    assert!(!text(&twin, "queries/NOTICE").contains(CONTEXT_REV));
    assert_eq!(
        listing(out.path()),
        ["c.tar.gz", "probe.tar.gz", "twin.tar.gz"]
    );
    assert_eq!(member(out.path(), "c"), b"an earlier build");
}

#[test]
fn a_language_that_fails_leaves_the_others_built() {
    let (grammar, grammar_rev) = grammar_repo(&[("LICENSE", b"MIT terms\n")]);
    let (not_a_grammar, not_a_grammar_rev) = repo(&[("README", b"no parser here\n")]);
    let sources = sources(&[
        ("probe", &file_url(grammar.path()), &grammar_rev),
        (
            "broken",
            &file_url(not_a_grammar.path()),
            &not_a_grammar_rev,
        ),
    ]);
    let languages = languages(&sources);
    let out = tempfile::tempdir().expect("out");

    let report = run(&languages, out.path()).expect("build");

    assert_eq!(report.built, ["probe"]);
    let failed: Vec<&str> = report
        .failed
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(failed, ["broken"]);
    assert!(
        report.failed[0].1.contains("parser.c"),
        "{:?}",
        report.failed
    );
    assert_eq!(listing(out.path()), ["probe.tar.gz"]);
}

#[test]
fn a_grammar_without_a_license_file_fails_and_leaves_the_others_built() {
    let (licensed, licensed_rev) = grammar_repo(&[("LICENSE", b"MIT terms\n")]);
    let (unlicensed, unlicensed_rev) = grammar_repo(&[]);
    let sources = sources(&[
        ("probe", &file_url(licensed.path()), &licensed_rev),
        ("twin", &file_url(unlicensed.path()), &unlicensed_rev),
    ]);
    let languages = languages(&sources);
    let out = tempfile::tempdir().expect("out");

    let report = run(&languages, out.path()).expect("build");

    assert_eq!(report.built, ["probe"]);
    assert_eq!(
        report.failed,
        [(
            "twin".to_string(),
            format!("no license file in the repository root at {unlicensed_rev}")
        )]
    );
}

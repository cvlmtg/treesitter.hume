use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use super::*;
use crate::pins::Pins;
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

/// A Helix tree whose `probe` highlights inherit `base`'s.
fn helix_repo() -> (tempfile::TempDir, String) {
    repo(&[
        (
            "runtime/queries/probe/highlights.scm",
            b"; inherits: base\n(word) @variable\n",
        ),
        (
            "runtime/queries/probe/textobjects.scm",
            b"(word) @word.inside\n",
        ),
        ("runtime/queries/twin/highlights.scm", b"(word) @string\n"),
        ("runtime/queries/broken/highlights.scm", b"(word) @string\n"),
        ("runtime/queries/base/highlights.scm", b"(source) @base\n"),
        ("languages.toml", b""),
    ])
}

fn pins(helix_rev: &str, languages: &[(&str, &str, &str)]) -> Pins {
    let mut toml = format!("helix-rev = \"{helix_rev}\"\n");
    for (name, url, rev) in languages {
        toml.push_str(&format!(
            "[[language]]\nname = \"{name}\"\nurl = \"{url}\"\nrev = \"{rev}\"\nsubpath = \"\"\nsymbol = \"tree_sitter_probe\"\n"
        ));
    }
    Pins::parse(&toml, "").expect("pins")
}

fn run(pins: &Pins, helix: &Path, out: &Path) -> Result<Report, BuildError> {
    let languages = ["probe", "twin", "broken"]
        .iter()
        .filter_map(|name| pins.get(name))
        .collect();
    build(
        &Request {
            languages,
            helix_url: &file_url(helix),
            helix_rev: pins.helix_rev(),
            out,
            tag: "v9.9.9",
            platform: "test-arch",
        },
        &Git::without_retries(),
    )
}

/// The member for `language`, read through the index as a client would.
fn member(out: &Path, language: &str) -> Vec<u8> {
    let index = fs::read_to_string(out.join("grammars-test-arch.index")).expect("index");
    let tar = fs::read(out.join("grammars-test-arch.tar")).expect("tar");
    let line = index
        .lines()
        .find(|line| line.starts_with(&format!("{language} ")))
        .unwrap_or_else(|| panic!("{language} in the index"));
    let fields: Vec<usize> = line
        .split(' ')
        .skip(1)
        .take(2)
        .map(|field| field.parse().expect("number"))
        .collect();
    tar[fields[0]..fields[0] + fields[1]].to_vec()
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
    let (helix, helix_rev) = helix_repo();
    let url = file_url(grammar.path());
    let pins = pins(
        &helix_rev,
        &[("probe", &url, &grammar_rev), ("twin", &url, &grammar_rev)],
    );
    let out = tempfile::tempdir().expect("out");

    let report = run(&pins, helix.path(), out.path()).expect("build");

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
            "queries/highlights.scm",
            "queries/textobjects.scm",
        ]
    );
    assert_eq!(text(&probe, "licenses/LICENSE"), "MIT terms\n");
    assert!(text(&probe, "licenses/SOURCE").contains(&grammar_rev));
    assert!(text(&probe, "queries/NOTICE").contains(&helix_rev));
    assert_eq!(
        text(&probe, "queries/highlights.scm"),
        "\n(source) @base\n\n(word) @variable\n"
    );
    let manifest = text(&probe, "manifest.scm");
    for field in [
        "(name . \"probe\")",
        "(symbol . \"tree_sitter_probe\")",
        "(abi . 15)",
        "(platform . \"test-arch\")",
        "(tag . \"v9.9.9\")",
        &format!("(helix-rev . \"{helix_rev}\")"),
    ] {
        assert!(manifest.contains(field), "{field} in {manifest}");
    }
    assert_eq!(
        text(
            &files(&member(out.path(), "twin")),
            "queries/highlights.scm"
        ),
        "(word) @string\n"
    );
    let sums = fs::read_to_string(out.path().join("SHA256SUMS")).expect("sums");
    let summed: Vec<&str> = sums.lines().map(|line| &line[66..]).collect();
    assert_eq!(
        summed,
        ["grammars-test-arch.index", "grammars-test-arch.tar"]
    );
}

#[test]
fn a_language_that_fails_leaves_the_others_built() {
    let (grammar, grammar_rev) = grammar_repo(&[("LICENSE", b"MIT terms\n")]);
    let (helix, helix_rev) = helix_repo();
    let pins = pins(
        &helix_rev,
        &[
            ("probe", &file_url(grammar.path()), &grammar_rev),
            ("broken", &file_url(helix.path()), &helix_rev),
        ],
    );
    let out = tempfile::tempdir().expect("out");

    let report = run(&pins, helix.path(), out.path()).expect("build");

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
    let index = fs::read_to_string(out.path().join("grammars-test-arch.index")).expect("index");
    assert_eq!(index.lines().count(), 2, "{index}");
}

#[test]
fn a_grammar_without_a_license_file_fails_and_leaves_the_others_built() {
    let (licensed, licensed_rev) = grammar_repo(&[("LICENSE", b"MIT terms\n")]);
    let (unlicensed, unlicensed_rev) = grammar_repo(&[]);
    let (helix, helix_rev) = helix_repo();
    let pins = pins(
        &helix_rev,
        &[
            ("probe", &file_url(licensed.path()), &licensed_rev),
            ("twin", &file_url(unlicensed.path()), &unlicensed_rev),
        ],
    );
    let out = tempfile::tempdir().expect("out");

    let report = run(&pins, helix.path(), out.path()).expect("build");

    assert_eq!(report.built, ["probe"]);
    assert_eq!(
        report.failed,
        [(
            "twin".to_string(),
            format!("no license file in the repository root at {unlicensed_rev}")
        )]
    );
}

#[test]
fn a_language_without_helix_highlights_stops_the_build_before_any_fetch() {
    let (helix, helix_rev) = helix_repo();
    let pins = pins(
        &helix_rev,
        &[
            ("missing", "file:///nonexistent", &helix_rev),
            ("probe", "file:///nonexistent", &helix_rev),
        ],
    );
    let out = tempfile::tempdir().expect("out");
    let languages = vec![
        pins.get("missing").expect("pinned"),
        pins.get("probe").expect("pinned"),
    ];

    let err = build(
        &Request {
            languages,
            helix_url: &file_url(helix.path()),
            helix_rev: pins.helix_rev(),
            out: out.path(),
            tag: "v9.9.9",
            platform: "test-arch",
        },
        &Git::without_retries(),
    )
    .expect_err("no highlights for missing");

    assert_eq!(err.to_string(), "Helix has no highlights.scm for: missing");
    assert_eq!(fs::read_dir(out.path()).expect("out").count(), 0);
}

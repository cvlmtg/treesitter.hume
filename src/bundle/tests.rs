use std::fs;
use std::io::Read;

use super::*;
use crate::manifest::Manifest;
use crate::sums::sha256_hex;

/// `archive`'s entries, as `(path, contents)` in archive order.
fn entries(archive: &[u8]) -> Vec<(String, Vec<u8>)> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    tar.entries()
        .expect("entries")
        .map(|entry| {
            let mut entry = entry.expect("entry");
            assert_eq!(entry.header().mtime().expect("mtime"), 0);
            let path = entry.path().expect("path").to_string_lossy().into_owned();
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents).expect("contents");
            (path, contents)
        })
        .collect()
}

#[test]
fn an_archive_holds_its_files_in_path_order() {
    let archive = archive(&[
        ("queries/highlights.scm".to_string(), b"(x)".to_vec()),
        ("c.so".to_string(), b"lib".to_vec()),
        ("manifest.scm".to_string(), b"()".to_vec()),
    ])
    .expect("archive");
    assert_eq!(
        entries(&archive),
        [
            ("c.so".to_string(), b"lib".to_vec()),
            ("manifest.scm".to_string(), b"()".to_vec()),
            ("queries/highlights.scm".to_string(), b"(x)".to_vec()),
        ]
    );
}

#[test]
fn an_archive_of_the_same_files_is_the_same_bytes() {
    let files = [("c.so".to_string(), b"lib".to_vec())];
    assert_eq!(
        archive(&files).expect("archive"),
        archive(&files).expect("archive")
    );
}

fn bundle(members: &[(&str, &[u8])]) -> Bundle {
    let mut bundle = Bundle::new("v1.0.0", "linux-x86_64");
    for (language, bytes) in members {
        bundle.insert(language, bytes.to_vec());
    }
    bundle
}

/// The members of the bundle in `dir`, read through its index alone: each
/// line's byte range of the tar must hash to the line's digest.
fn members_by_index(dir: &Path) -> Vec<(String, Vec<u8>)> {
    let index = fs::read_to_string(dir.join("grammars-linux-x86_64.index")).expect("index");
    let tar = fs::read(dir.join("grammars-linux-x86_64.tar")).expect("tar");
    let mut lines = index.lines();
    assert_eq!(lines.next(), Some("treesitter.hume v1.0.0 linux-x86_64"));
    lines
        .map(|line| {
            let [language, offset, size, digest] = line.split(' ').collect::<Vec<_>>()[..] else {
                panic!("index line {line:?}");
            };
            let offset: usize = offset.parse().expect("offset");
            let size: usize = size.parse().expect("size");
            let bytes = tar[offset..offset + size].to_vec();
            assert_eq!(sha256_hex(&bytes), digest, "{language}");
            (language.to_string(), bytes)
        })
        .collect()
}

#[test]
fn each_index_line_locates_its_member_in_the_tar() {
    let dir = tempfile::tempdir().expect("dir");
    bundle(&[("rust", b"rust archive"), ("c", &[7; 1500])])
        .write(dir.path())
        .expect("write");
    assert_eq!(
        members_by_index(dir.path()),
        [
            ("c".to_string(), vec![7; 1500]),
            ("rust".to_string(), b"rust archive".to_vec()),
        ]
    );
}

#[test]
fn the_tar_names_each_member_after_its_language() {
    let dir = tempfile::tempdir().expect("dir");
    bundle(&[("rust", b"r"), ("markdown.inline", b"m")])
        .write(dir.path())
        .expect("write");
    let tar = fs::File::open(dir.path().join("grammars-linux-x86_64.tar")).expect("tar");
    let names: Vec<String> = tar::Archive::new(tar)
        .entries()
        .expect("entries")
        .map(|entry| {
            entry
                .expect("entry")
                .path()
                .expect("path")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["markdown.inline.tar.gz", "rust.tar.gz"]);
}

/// A language archive whose manifest names `name`, `platform` and `tag`.
fn built(name: &str, platform: &str, tag: &str) -> Vec<u8> {
    let manifest = Manifest {
        name,
        url: "https://example.com/tree-sitter-x",
        rev: "abc",
        subpath: "",
        symbol: "tree_sitter_x",
        abi: 15,
        platform,
        tag,
        helix_rev: "def",
    };
    archive(&[(
        "manifest.scm".to_string(),
        manifest.to_scheme().into_bytes(),
    )])
    .expect("archive")
}

#[test]
fn reading_a_folder_takes_each_archive_under_its_language() {
    let grammars = tempfile::tempdir().expect("grammars");
    let c = built("c", "linux-x86_64", "v1.0.0");
    let inline = built("markdown.inline", "linux-x86_64", "v1.0.0");
    write_archive(grammars.path(), "c", &c).expect("c");
    write_archive(grammars.path(), "markdown.inline", &inline).expect("inline");
    fs::write(grammars.path().join("notes.txt"), "not an archive").expect("notes");
    let out = tempfile::tempdir().expect("out");

    Bundle::read(grammars.path(), "v1.0.0", "linux-x86_64")
        .expect("read")
        .write(out.path())
        .expect("write");

    assert_eq!(
        members_by_index(out.path()),
        [
            ("c".to_string(), c),
            ("markdown.inline".to_string(), inline),
        ]
    );
}

/// The error reading a folder that holds only `archive`, as `language`.
fn read_error(language: &str, archive: &[u8]) -> String {
    let grammars = tempfile::tempdir().expect("grammars");
    write_archive(grammars.path(), language, archive).expect("archive");
    Bundle::read(grammars.path(), "v1.0.0", "linux-x86_64")
        .expect_err("refused")
        .to_string()
}

#[test]
fn an_archive_from_another_tag_is_refused() {
    let err = read_error("c", &built("c", "linux-x86_64", "v0.9.0"));
    assert!(err.contains("v0.9.0"), "{err}");
}

#[test]
fn an_archive_for_another_platform_is_refused() {
    let err = read_error("c", &built("c", "macos-aarch64", "v1.0.0"));
    assert!(err.contains("macos-aarch64"), "{err}");
}

#[test]
fn an_archive_of_another_language_is_refused() {
    let err = read_error("rust", &built("c", "linux-x86_64", "v1.0.0"));
    assert!(err.contains("rust.tar.gz"), "{err}");
}

#[test]
fn an_archive_without_a_manifest_is_refused() {
    let archive = archive(&[("c.so".to_string(), b"lib".to_vec())]).expect("archive");
    let err = read_error("c", &archive);
    assert!(err.contains("manifest.scm"), "{err}");
}

#[test]
fn a_folder_without_archives_is_refused() {
    let grammars = tempfile::tempdir().expect("grammars");
    Bundle::read(grammars.path(), "v1.0.0", "linux-x86_64").expect_err("no archives");
}

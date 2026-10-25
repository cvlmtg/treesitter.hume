use std::fs;
use std::io::Read;

use super::*;
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

#[test]
fn opening_an_existing_bundle_keeps_its_members() {
    let dir = tempfile::tempdir().expect("dir");
    bundle(&[("c", b"old c"), ("rust", b"rust")])
        .write(dir.path())
        .expect("write");
    let mut reopened = Bundle::open(dir.path(), "v1.0.0", "linux-x86_64").expect("open");
    reopened.insert("c", b"new c".to_vec());
    reopened.insert("json", b"json".to_vec());
    reopened.write(dir.path()).expect("write");
    assert_eq!(
        members_by_index(dir.path()),
        [
            ("c".to_string(), b"new c".to_vec()),
            ("json".to_string(), b"json".to_vec()),
            ("rust".to_string(), b"rust".to_vec()),
        ]
    );
}

#[test]
fn opening_an_empty_directory_starts_an_empty_bundle() {
    let dir = tempfile::tempdir().expect("dir");
    let bundle = Bundle::open(dir.path(), "v1.0.0", "linux-x86_64").expect("open");
    assert!(bundle.members.is_empty());
}

#[test]
fn a_bundle_from_another_tag_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    bundle(&[("c", b"c")]).write(dir.path()).expect("write");
    let err = Bundle::open(dir.path(), "v2.0.0", "linux-x86_64").expect_err("older tag");
    assert!(err.to_string().contains("v1.0.0"), "{err}");
}

#[test]
fn a_tar_without_its_index_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    bundle(&[("c", b"c")]).write(dir.path()).expect("write");
    fs::remove_file(dir.path().join("grammars-linux-x86_64.index")).expect("remove");
    Bundle::open(dir.path(), "v1.0.0", "linux-x86_64").expect_err("no index");
}

#[test]
fn a_member_that_does_not_match_its_digest_is_refused() {
    let dir = tempfile::tempdir().expect("dir");
    bundle(&[("c", b"c archive")])
        .write(dir.path())
        .expect("write");
    let path = dir.path().join("grammars-linux-x86_64.tar");
    let mut tar = fs::read(&path).expect("tar");
    let at = tar
        .windows(9)
        .position(|w| w == b"c archive")
        .expect("member bytes");
    tar[at] = b'C';
    fs::write(&path, tar).expect("tar");
    let err = Bundle::open(dir.path(), "v1.0.0", "linux-x86_64").expect_err("corrupt");
    assert!(err.to_string().contains("c"), "{err}");
}

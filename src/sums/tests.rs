use std::fs;

use super::*;

const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

#[test]
fn every_file_is_listed_in_name_order_with_its_sha256() {
    let dir = tempfile::tempdir().expect("dir");
    fs::write(dir.path().join("b.index"), "abc").expect("file");
    fs::write(dir.path().join("a.tar"), "").expect("file");
    write(dir.path()).expect("sums");
    assert_eq!(
        fs::read_to_string(dir.path().join(SUMS)).expect("sums file"),
        format!("{EMPTY}  a.tar\n{ABC}  b.index\n")
    );
}

#[test]
fn the_sums_file_dotfiles_and_directories_are_left_out() {
    let dir = tempfile::tempdir().expect("dir");
    fs::write(dir.path().join("a.tar"), "abc").expect("file");
    fs::write(dir.path().join(".DS_Store"), "x").expect("file");
    fs::create_dir(dir.path().join("sub")).expect("dir");
    fs::write(dir.path().join(SUMS), "stale").expect("file");
    write(dir.path()).expect("sums");
    write(dir.path()).expect("sums again");
    assert_eq!(
        fs::read_to_string(dir.path().join(SUMS)).expect("sums file"),
        format!("{ABC}  a.tar\n")
    );
}

#[test]
fn sha256_hex_matches_known_digests() {
    assert_eq!(sha256_hex(b""), EMPTY);
    assert_eq!(sha256_hex(b"abc"), ABC);
}

use std::ffi::OsString;
use std::path::PathBuf;

use super::*;

fn parsed(args: &[&str]) -> Result<Command, String> {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    parse(&args)
}

fn languages(names: &[&str]) -> Selection {
    Selection::Languages(names.iter().map(|name| name.to_string()).collect())
}

#[test]
fn build_takes_languages_and_writes_to_dist_by_default() {
    assert_eq!(
        parsed(&["build", "rust", "c"]),
        Ok(Command::Build {
            out: PathBuf::from("dist"),
            selection: languages(&["rust", "c"]),
        })
    );
}

#[test]
fn build_takes_an_output_directory_and_all_languages() {
    assert_eq!(
        parsed(&["build", "--out", "out", "--all"]),
        Ok(Command::Build {
            out: PathBuf::from("out"),
            selection: Selection::All,
        })
    );
}

#[test]
fn build_needs_languages_or_all_but_not_both() {
    assert!(parsed(&["build"]).is_err());
    assert!(parsed(&["build", "--all", "rust"]).is_err());
    assert!(parsed(&["build", "--out"]).is_err());
    assert!(parsed(&["build", "--bogus", "rust"]).is_err());
}

#[test]
fn sums_and_compile_take_their_paths() {
    assert_eq!(
        parsed(&["sums", "dist"]),
        Ok(Command::Sums(PathBuf::from("dist")))
    );
    assert_eq!(
        parsed(&["compile", "grammar", "lib.so"]),
        Ok(Command::Compile {
            grammar: PathBuf::from("grammar"),
            library: PathBuf::from("lib.so"),
        })
    );
    assert!(parsed(&["compile", "grammar"]).is_err());
}

#[test]
fn bundle_takes_the_grammars_and_output_directories() {
    assert_eq!(
        parsed(&["bundle", "grammars", "dist"]),
        Ok(Command::Bundle {
            grammars: PathBuf::from("grammars"),
            out: PathBuf::from("dist"),
        })
    );
    assert!(parsed(&["bundle", "grammars"]).is_err());
}

#[test]
fn version_and_unknown_commands() {
    assert_eq!(parsed(&["--version"]), Ok(Command::Version));
    assert!(parsed(&[]).is_err());
    assert!(parsed(&["-o", "lib.so", "grammar"]).is_err());
}

#[test]
fn a_named_language_must_be_pinned() {
    let pins = Pins::embedded().expect("pins");
    let err = selected(
        &pins,
        &languages(&["rust", "cobol", "klingon"]),
        "linux-x86_64",
    )
    .expect_err("unpinned names");
    assert_eq!(err, "not pinned: cobol, klingon");
}

#[test]
fn a_language_named_twice_is_built_once() {
    let pins = Pins::embedded().expect("pins");
    let chosen =
        selected(&pins, &languages(&["rust", "c", "rust"]), "linux-x86_64").expect("pinned names");
    let names: Vec<&str> = chosen.iter().map(|pin| pin.name.as_str()).collect();
    assert_eq!(names, ["c", "rust"]);
}

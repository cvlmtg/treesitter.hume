use std::ffi::OsString;
use std::path::PathBuf;

use super::*;

#[test]
fn zig_is_recognised_by_its_program_name() {
    assert!(is_zig("zig"));
    assert!(is_zig("/opt/zig-0.16/zig"));
    assert!(is_zig("zig.exe"));
}

/// A wrapper in front of zig is the program, so the build goes through `cc`.
#[test]
fn other_programs_are_not_zig() {
    assert!(!is_zig("cc"));
    assert!(!is_zig("ccache"));
    assert!(!is_zig("zigcc"));
}

#[test]
fn the_driver_keeps_every_argument_and_optimizes() {
    let compiler = driver("zig", &["c++", "-target", "x86_64-linux-gnu.2.28"]);
    assert_eq!(compiler.program, PathBuf::from("zig"));
    assert_eq!(
        compiler.args,
        ["c++", "-target", "x86_64-linux-gnu.2.28", "-O2"].map(OsString::from)
    );
    assert!(compiler.env.is_empty());
    assert_eq!(compiler.flavor, Flavor::Gnu);
    assert_eq!(compiler.finalizer, finalizer(TARGET));
}

#[test]
fn glibc_targets_link_the_finalizer() {
    for target in [
        "x86_64-unknown-linux-gnu",
        "aarch64-unknown-linux-gnu",
        "armv7-unknown-linux-gnueabihf",
    ] {
        assert_eq!(finalizer(target), Some(FINALIZER), "{target}");
    }
}

#[test]
fn other_targets_link_no_finalizer() {
    for target in [
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "x86_64-pc-windows-gnu",
        "x86_64-unknown-linux-musl",
    ] {
        assert_eq!(finalizer(target), None, "{target}");
    }
}

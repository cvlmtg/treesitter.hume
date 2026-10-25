use std::ffi::OsString;

use super::*;

/// A lookup over `vars` only, standing in for the process environment.
fn vars<'a>(vars: &'a [(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
    move |key| {
        vars.iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| OsString::from(value))
    }
}

fn words(words: &[&str]) -> Option<EnvCompiler> {
    let (program, args) = words.split_first().expect("a program");
    Some(EnvCompiler {
        program: program.to_string(),
        args: args.iter().map(|arg| arg.to_string()).collect(),
    })
}

#[test]
fn compiler_variables_are_read_in_cc_order() {
    let target = format!("CC_{TARGET}");
    let target_underscored = format!("CC_{}", TARGET.replace(['-', '.'], "_"));
    let order = [target.as_str(), &target_underscored, "HOST_CC", "CC"];
    for (i, winner) in order.iter().enumerate() {
        let set: Vec<(&str, &str)> = order[i..].iter().map(|name| (*name, *name)).collect();
        assert_eq!(
            env_compiler(false, vars(&set)),
            words(&[winner]),
            "{winner}"
        );
    }
}

#[test]
fn a_cpp_build_reads_the_cxx_variables() {
    let set = [("CC", "gcc"), ("HOST_CXX", "g++ -O1"), ("CXX", "c++")];
    assert_eq!(env_compiler(true, vars(&set)), words(&["g++", "-O1"]));
}

#[test]
fn no_compiler_variable_means_no_override() {
    assert_eq!(env_compiler(false, vars(&[("CXX", "c++")])), None);
}

/// `cc` stops at the first variable that is set, and an empty one names
/// no compiler, so a later variable is not consulted.
#[test]
fn an_empty_compiler_variable_means_no_override() {
    let set = [("HOST_CC", " "), ("CC", "zig cc")];
    assert_eq!(env_compiler(false, vars(&set)), None);
}

/// A path with spaces that names an existing file is one word.
#[test]
fn an_existing_path_is_one_word() {
    let dir = tempfile::tempdir().expect("temp dir");
    let compiler = dir.path().join("my zig");
    std::fs::write(&compiler, "").expect("compiler file");
    let compiler = compiler.to_str().expect("UTF-8 temp path");
    assert_eq!(
        env_compiler(false, vars(&[("CC", compiler)])),
        words(&[compiler])
    );
}

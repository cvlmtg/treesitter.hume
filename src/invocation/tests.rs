use std::path::PathBuf;

use super::*;

fn sources(scanner: Option<Scanner>) -> Sources {
    Sources {
        dir: PathBuf::from("/g/src"),
        parser: PathBuf::from("/g/src/parser.c"),
        scanner,
    }
}

fn strings(steps: Vec<Vec<OsString>>) -> Vec<Vec<String>> {
    steps
        .into_iter()
        .map(|step| {
            step.into_iter()
                .map(|arg| arg.into_string().expect("test paths are UTF-8"))
                .collect()
        })
        .collect()
}

/// The flags every GNU step starts with.
fn gnu_common() -> Vec<String> {
    let mut common = vec!["-fno-exceptions".to_string()];
    if cfg!(not(windows)) {
        common.push("-fPIC".to_string());
    }
    common.extend(["-I", "/g/src"].map(String::from));
    common
}

fn gnu_link(objects_and_sources: &[&str]) -> Vec<String> {
    let mut link = vec!["-shared".to_string()];
    link.extend(gnu_common());
    link.extend(["-o", "/w/x.so"].map(String::from));
    link.extend(objects_and_sources.iter().map(|s| s.to_string()));
    if cfg!(target_os = "linux") {
        link.push("-Wl,-z,relro,-z,now".to_string());
    }
    link
}

#[test]
fn gnu_without_scanner_links_the_parser_alone() {
    let steps = steps(
        Flavor::Gnu,
        &sources(None),
        None,
        Path::new("/w"),
        Path::new("/w/x.so"),
    );
    assert_eq!(
        strings(steps),
        vec![gnu_link(&["-xc", "-std=c11", "/g/src/parser.c"])]
    );
}

#[test]
fn gnu_c_scanner_is_compiled_with_the_parser() {
    let scanner = Some(Scanner::C(PathBuf::from("/g/src/scanner.c")));
    let steps = steps(
        Flavor::Gnu,
        &sources(scanner),
        None,
        Path::new("/w"),
        Path::new("/w/x.so"),
    );
    assert_eq!(
        strings(steps),
        vec![gnu_link(&[
            "-xc",
            "-std=c11",
            "/g/src/scanner.c",
            "/g/src/parser.c"
        ])]
    );
}

#[test]
fn gnu_cpp_scanner_is_an_object_linked_before_the_c_sources() {
    let scanner = Some(Scanner::Cpp(PathBuf::from("/g/src/scanner.cc")));
    let steps = steps(
        Flavor::Gnu,
        &sources(scanner),
        None,
        Path::new("/w"),
        Path::new("/w/x.so"),
    );
    let object = Path::new("/w").join("scanner.o");
    let object = object.to_str().expect("test paths are UTF-8");
    let mut compile = gnu_common();
    compile.extend(["-std=c++14", "-c", "/g/src/scanner.cc", "-o", object].map(String::from));
    assert_eq!(
        strings(steps),
        vec![
            compile,
            gnu_link(&[object, "-xc", "-std=c11", "/g/src/parser.c"])
        ]
    );
}

#[test]
fn gnu_finalizer_is_compiled_with_the_c_sources() {
    let steps = steps(
        Flavor::Gnu,
        &sources(None),
        Some(Path::new("/w/finalize.c")),
        Path::new("/w"),
        Path::new("/w/x.so"),
    );
    assert_eq!(
        strings(steps),
        vec![gnu_link(&[
            "-xc",
            "-std=c11",
            "/w/finalize.c",
            "/g/src/parser.c"
        ])]
    );
}

#[test]
fn msvc_c_scanner_is_compiled_with_the_parser() {
    let scanner = Some(Scanner::C(PathBuf::from("/g/src/scanner.c")));
    let steps = steps(
        Flavor::Msvc,
        &sources(scanner),
        None,
        Path::new("/w"),
        Path::new("/w/x.dll"),
    );
    assert_eq!(
        strings(steps),
        vec![
            [
                "/nologo",
                "/utf-8",
                "/I",
                "/g/src",
                "/LD",
                "/std:c11",
                "/g/src/scanner.c",
                "/g/src/parser.c",
                "/link",
                "/out:/w/x.dll",
            ]
            .map(String::from)
            .to_vec()
        ]
    );
}

#[test]
fn msvc_cpp_scanner_is_an_object_linked_with_the_parser() {
    let scanner = Some(Scanner::Cpp(PathBuf::from("/g/src/scanner.cc")));
    let steps = steps(
        Flavor::Msvc,
        &sources(scanner),
        None,
        Path::new("/w"),
        Path::new("/w/x.dll"),
    );
    let object = Path::new("/w").join("scanner.obj");
    let object = object.to_str().expect("test paths are UTF-8");
    assert_eq!(
        strings(steps),
        vec![
            [
                "/nologo",
                "/utf-8",
                "/I",
                "/g/src",
                "/std:c++14",
                "/c",
                "/g/src/scanner.cc",
                &format!("/Fo{object}"),
            ]
            .map(String::from)
            .to_vec(),
            [
                "/nologo",
                "/utf-8",
                "/I",
                "/g/src",
                "/LD",
                "/std:c11",
                object,
                "/g/src/parser.c",
                "/link",
                "/out:/w/x.dll",
            ]
            .map(String::from)
            .to_vec()
        ]
    );
}

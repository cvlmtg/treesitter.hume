//! `hume-grammar-build`: builds tree-sitter grammars into the archives HUME
//! installs, for the platform it runs on.

#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]

mod bundle;
mod compile;
mod git;
mod invocation;
mod licenses;
mod manifest;
mod pins;
mod pipeline;
mod platform;
mod probe;
mod queries;
mod sources;
mod sums;
#[cfg(test)]
mod test_support;
mod toolchain;

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use git::Git;
use pins::{Pin, Pins};
use pipeline::Request;

const USAGE: &str = "\
usage: hume-grammar-build build [--out <dir>] (--all | <language>...)
       hume-grammar-build bundle <grammars-dir> <out-dir>
       hume-grammar-build sums <dir>
       hume-grammar-build compile <grammar-dir> <library>
       hume-grammar-build --version";

/// The release tag of this build of the tool.
const TAG: &str = concat!("v", env!("CARGO_PKG_VERSION"));

#[derive(Debug, PartialEq, Eq)]
enum Command {
    /// Build pinned languages into `out`, one archive each.
    Build {
        out: PathBuf,
        selection: Selection,
    },
    /// Pack the language archives in `grammars` into a release bundle in
    /// `out`, with its `SHA256SUMS`.
    Bundle {
        grammars: PathBuf,
        out: PathBuf,
    },
    /// Write `SHA256SUMS` for the files in a directory.
    Sums(PathBuf),
    /// Compile one grammar checkout into a library, for a grammar HUME does
    /// not pin.
    Compile {
        grammar: PathBuf,
        library: PathBuf,
    },
    Version,
}

#[derive(Debug, PartialEq, Eq)]
enum Selection {
    /// Every pinned language this platform's exclusions leave in.
    All,
    Languages(Vec<String>),
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let command = match parse(&args) {
        Ok(command) => command,
        Err(problem) => {
            eprintln!("hume-grammar-build: {problem}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(command) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("hume-grammar-build: {e}");
            ExitCode::FAILURE
        }
    }
}

fn parse(args: &[OsString]) -> Result<Command, String> {
    let strings: Vec<&str> = args
        .iter()
        .map(|arg| arg.to_str().ok_or_else(|| format!("{arg:?} is not UTF-8")))
        .collect::<Result<_, _>>()?;
    match strings.as_slice() {
        ["--version"] => Ok(Command::Version),
        ["sums", dir] => Ok(Command::Sums(PathBuf::from(dir))),
        ["bundle", grammars, out] => Ok(Command::Bundle {
            grammars: PathBuf::from(grammars),
            out: PathBuf::from(out),
        }),
        ["compile", grammar, library] => Ok(Command::Compile {
            grammar: PathBuf::from(grammar),
            library: PathBuf::from(library),
        }),
        ["build", rest @ ..] => parse_build(rest),
        _ => Err("expected a command".to_string()),
    }
}

fn parse_build(args: &[&str]) -> Result<Command, String> {
    let mut out = PathBuf::from("dist");
    let mut all = false;
    let mut names = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match *arg {
            "--out" => out = PathBuf::from(args.next().ok_or("--out needs a directory")?),
            "--all" => all = true,
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            name => names.push(name.to_string()),
        }
    }
    let selection = match (all, names.is_empty()) {
        (true, true) => Selection::All,
        (false, false) => Selection::Languages(names),
        (true, false) => return Err("--all takes no language names".to_string()),
        (false, true) => return Err("name the languages to build, or pass --all".to_string()),
    };
    Ok(Command::Build { out, selection })
}

fn run(command: Command) -> Result<ExitCode, String> {
    match command {
        Command::Version => {
            println!("hume-grammar-build {TAG}");
            Ok(ExitCode::SUCCESS)
        }
        Command::Sums(dir) => sums::write(&dir)
            .map(|()| ExitCode::SUCCESS)
            .map_err(|e| format!("{}: {e}", dir.display())),
        Command::Bundle { grammars, out } => {
            let platform = platform::current();
            let bundle =
                bundle::Bundle::read(&grammars, TAG, &platform).map_err(|e| e.to_string())?;
            bundle.write(&out).map_err(|e| e.to_string())?;
            sums::write(&out).map_err(|e| format!("{}: {e}", out.display()))?;
            eprintln!(
                "bundled {} for {platform} into {}",
                bundle.len(),
                out.display()
            );
            Ok(ExitCode::SUCCESS)
        }
        Command::Compile { grammar, library } => compile::build(&grammar, &library)
            .map(|()| ExitCode::SUCCESS)
            .map_err(|e| e.to_string()),
        Command::Build { out, selection } => {
            let pins = Pins::embedded().map_err(|e| e.to_string())?;
            let platform = platform::current();
            let languages = selected(&pins, &selection, &platform)?;
            let report = pipeline::build(
                &Request {
                    languages,
                    helix_url: pipeline::HELIX_URL,
                    helix_rev: pins.helix_rev(),
                    out: &out,
                    tag: TAG,
                    platform: &platform,
                },
                &Git::new(),
            )
            .map_err(|e| e.to_string())?;
            eprintln!(
                "built {} for {platform} into {}",
                report.built.len(),
                out.display()
            );
            if report.failed.is_empty() {
                return Ok(ExitCode::SUCCESS);
            }
            let failed: Vec<&str> = report
                .failed
                .iter()
                .map(|(name, _)| name.as_str())
                .collect();
            eprintln!("failed {}: {}", failed.len(), failed.join(", "));
            Ok(ExitCode::FAILURE)
        }
    }
}

/// The pins `selection` names, each once and in name order. Every named
/// language must be pinned.
fn selected<'a>(
    pins: &'a Pins,
    selection: &Selection,
    platform: &str,
) -> Result<Vec<&'a Pin>, String> {
    let names = match selection {
        Selection::All => return Ok(pins.release(platform)),
        Selection::Languages(names) => names.iter().map(String::as_str).collect::<BTreeSet<_>>(),
    };
    let unknown: Vec<&str> = names
        .iter()
        .filter(|name| pins.get(name).is_none())
        .copied()
        .collect();
    if !unknown.is_empty() {
        return Err(format!("not pinned: {}", unknown.join(", ")));
    }
    Ok(names.iter().filter_map(|name| pins.get(name)).collect())
}

#[cfg(test)]
mod tests;

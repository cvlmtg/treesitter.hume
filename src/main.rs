//! `hume-grammar-build -o <library> <grammar-dir>`: HUME runs this to
//! compile a grammar it installs. It builds the generated `parser.c`, plus
//! an external scanner written in C or C++, into the shared library HUME
//! loads.

#![deny(rustdoc::broken_intra_doc_links)]
#![deny(rustdoc::private_intra_doc_links)]

mod compile;
mod git;
mod invocation;
mod manifest;
mod pins;
mod probe;
mod queries;
mod sources;
#[cfg(test)]
mod test_support;
mod toolchain;

use std::path::Path;
use std::process::ExitCode;

use compile::build;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [flag, out, grammar_dir] = args.as_slice() else {
        return usage();
    };
    if flag != "-o" {
        return usage();
    }
    match build(Path::new(grammar_dir), Path::new(out)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("hume-grammar-build: {e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: hume-grammar-build -o <library> <grammar-dir>");
    ExitCode::from(2)
}

//! `zig cc` and `zig c++`, run directly.
//!
//! `cc` treats zig as clang-family and passes it `--target=<rust triple>`,
//! which zig rejects, so a zig driver runs with its own arguments in place
//! of `cc`'s. Its target, such as a glibc version suffix, comes from the
//! arguments that name it.
//!
//! A zig link for glibc has no `crtbeginS.o`, so each library also gets
//! `finalize.c`, which runs the library's exit handlers when it is
//! unloaded.

use std::ffi::OsStr;
use std::path::Path;

use super::{Compiler, Flavor, TARGET};

const FINALIZER: &str = include_str!("zig/finalize.c");

/// Whether `program` is zig. A wrapper in front of it, such as
/// `ccache zig cc`, names the wrapper instead and leaves the build to `cc`.
pub(super) fn is_zig(program: &str) -> bool {
    Path::new(program).file_stem() == Some(OsStr::new("zig"))
}

/// Zig at `program`, run with `args` (`cc` or `c++` first), optimizing as
/// `cc` would.
pub(super) fn driver(program: &str, args: &[impl AsRef<OsStr>]) -> Compiler {
    Compiler {
        program: program.into(),
        args: args
            .iter()
            .map(|arg| arg.as_ref().to_owned())
            .chain(["-O2".into()])
            .collect(),
        env: Vec::new(),
        flavor: Flavor::Gnu,
        finalizer: finalizer(TARGET),
    }
}

/// The finalizer a library built for the Rust triple `target` links, which
/// every glibc target needs.
fn finalizer(target: &str) -> Option<&'static str> {
    target.contains("-linux-gnu").then_some(FINALIZER)
}

#[cfg(test)]
mod tests;

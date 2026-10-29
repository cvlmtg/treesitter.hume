//! A compiler for Windows machines without MSVC.
//!
//! `cc` assumes MSVC on an `-msvc` target and names `cl.exe` even when it
//! is not installed. It also passes clang `--target=<host LLVM triple>`,
//! which forces the MSVC ABI onto a clang paired with MinGW. A fallback
//! driver here runs with its own arguments in place of `cc`'s, so each
//! compiler keeps its own native target.

use std::io;
use std::process::Command;

use super::{Compiler, Flavor, TARGET, zig};
use crate::compile::CompileError;

/// A compiler found on `PATH`, in the order they are preferred.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WindowsCompiler {
    Msvc,
    Clang,
    Gcc,
    Zig,
}

/// The first compiler `exists` reports, `cl` standing for MSVC.
/// `exists` is injected so the order is testable without a real `PATH`.
fn choose_windows_compiler(exists: impl Fn(&str) -> bool) -> Option<WindowsCompiler> {
    [
        ("cl", WindowsCompiler::Msvc),
        ("clang", WindowsCompiler::Clang),
        ("gcc", WindowsCompiler::Gcc),
        ("zig", WindowsCompiler::Zig),
    ]
    .into_iter()
    .find(|(name, _)| exists(name))
    .map(|(_, compiler)| compiler)
}

/// The driver for `compiler` in place of `cc`, or `None` for MSVC, which
/// `cc` drives itself.
///
/// The arguments stand in for those `cc` would add: an optimization level,
/// and for g++ a static C++ runtime, so the DLL does not need MinGW's
/// runtime DLLs on `PATH` when HUME loads it.
fn fallback_driver(compiler: WindowsCompiler, cpp: bool) -> Option<Compiler> {
    let gnu = |program: &str, args: &[&str]| Compiler {
        program: program.into(),
        args: args.iter().map(Into::into).collect(),
        env: Vec::new(),
        flavor: Flavor::Gnu,
        finalizer: None,
    };
    match (compiler, cpp) {
        (WindowsCompiler::Msvc, _) => None,
        (WindowsCompiler::Clang, false) => Some(gnu("clang", &["-O2"])),
        (WindowsCompiler::Clang, true) => Some(gnu("clang++", &["-O2"])),
        (WindowsCompiler::Gcc, false) => Some(gnu("gcc", &["-O2"])),
        (WindowsCompiler::Gcc, true) => {
            Some(gnu("g++", &["-O2", "-static-libgcc", "-static-libstdc++"]))
        }
        (WindowsCompiler::Zig, false) => Some(zig::driver("zig", &["cc"])),
        (WindowsCompiler::Zig, true) => Some(zig::driver("zig", &["c++"])),
    }
}

/// A non-MSVC driver, or `None` to let `cc` drive MSVC when it is
/// installed. Only for an environment that names no compiler.
pub(super) fn without_msvc(cpp: bool) -> Result<Option<Compiler>, CompileError> {
    let exists = |name: &str| {
        (name == "cl" && cc::windows_registry::find_tool(TARGET, "cl.exe").is_some())
            || exe_on_path(name)
    };
    let compiler = choose_windows_compiler(exists).ok_or_else(|| {
        CompileError::NoCompiler(
            "install one of: MSVC Build Tools, clang, gcc, or zig, and ensure it is on PATH"
                .to_string(),
        )
    })?;
    Ok(fallback_driver(compiler, cpp))
}

/// Whether `name` resolves to a runnable executable on `PATH`.
///
/// Spawns `name --version` and treats `NotFound` as absent; any other
/// outcome (success, nonzero exit, permission error) means the executable
/// exists. This respects `PATHEXT` without an extra dependency.
fn exe_on_path(name: &str) -> bool {
    match Command::new(name).arg("--version").output() {
        Ok(_) => true,
        Err(e) => e.kind() != io::ErrorKind::NotFound,
    }
}

#[cfg(test)]
mod tests;

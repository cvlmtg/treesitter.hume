use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;

use crate::invocation;
use crate::sources::{Scanner, Sources};
use crate::toolchain::Compiler;

/// Why a grammar did not build.
#[derive(Debug)]
pub(crate) enum CompileError {
    NoParser(PathBuf),
    NoFileName(PathBuf),
    NoCompiler(String),
    Spawn {
        program: PathBuf,
        source: io::Error,
    },
    /// The compiler ran and failed; `output` is what it printed.
    Failed {
        program: PathBuf,
        status: ExitStatus,
        output: String,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for CompileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoParser(path) => write!(f, "no grammar parser at {}", path.display()),
            Self::NoFileName(path) => write!(f, "output path {} names no file", path.display()),
            Self::NoCompiler(reason) => write!(f, "no usable compiler: {reason}"),
            Self::Spawn { program, source } => {
                write!(f, "cannot run {}: {source}", program.display())
            }
            Self::Failed {
                program,
                status,
                output,
            } => {
                write!(
                    f,
                    "{} failed ({status}):\n{}",
                    program.display(),
                    output.trim_end()
                )
            }
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

/// A [`CompileError::Io`] on `path`, for `map_err`.
fn io_error(path: &Path) -> impl FnOnce(io::Error) -> CompileError {
    let path = path.to_path_buf();
    move |source| CompileError::Io { path, source }
}

/// Build the grammar rooted at `grammar_dir` into the shared library `out`.
///
/// The library is linked in a scratch directory beside `out` and renamed
/// over it, so `out` is never a partly written file: a failed rebuild leaves
/// the previous library loadable.
pub(crate) fn build(grammar_dir: &Path, out: &Path) -> Result<(), CompileError> {
    let grammar_dir = std::path::absolute(grammar_dir).map_err(io_error(grammar_dir))?;
    let out = std::path::absolute(out).map_err(io_error(out))?;
    let file_name = out
        .file_name()
        .ok_or_else(|| CompileError::NoFileName(out.clone()))?;
    let out_dir = out
        .parent()
        .ok_or_else(|| CompileError::NoFileName(out.clone()))?;

    let sources = Sources::find(&grammar_dir)?;
    let compiler = Compiler::find(matches!(sources.scanner, Some(Scanner::Cpp(_))))?;

    std::fs::create_dir_all(out_dir).map_err(io_error(out_dir))?;
    let work = tempfile::Builder::new()
        .prefix(".hume-grammar-build")
        .tempdir_in(out_dir)
        .map_err(io_error(out_dir))?;
    let library = work.path().join(file_name);
    let finalizer = match compiler.finalizer() {
        Some(source) => {
            let path = work.path().join("finalize.c");
            std::fs::write(&path, source).map_err(io_error(&path))?;
            Some(path)
        }
        None => None,
    };
    for args in invocation::steps(
        compiler.flavor(),
        &sources,
        finalizer.as_deref(),
        work.path(),
        &library,
    ) {
        compiler.run(&args, work.path())?;
    }
    std::fs::rename(&library, &out).map_err(io_error(&out))
}

#[cfg(test)]
mod tests;

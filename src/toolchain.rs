use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::compile::CompileError;

/// The triple this binary was built for, which is also the one it builds
/// grammars for: they are loaded into a HUME running on the same machine.
const TARGET: &str = env!("HUME_GRAMMAR_BUILD_TARGET");

/// How a compiler driver spells its flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flavor {
    Gnu,
    Msvc,
}

/// A C or C++ compiler driver, with the arguments and environment it runs with.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Compiler {
    program: PathBuf,
    args: Vec<OsString>,
    /// `None` removes the variable.
    env: Vec<(OsString, Option<OsString>)>,
    flavor: Flavor,
}

impl Compiler {
    /// The C++ driver when `cpp`, else the C driver. `CC`/`CXX` override the
    /// platform default, as they do for `cc`'s build scripts, and one that
    /// names zig runs without `cc` (see [`zig`]).
    pub(crate) fn find(cpp: bool) -> Result<Self, CompileError> {
        let named = env_compiler(cpp, |name| std::env::var_os(name));
        if let Some(named) = &named
            && zig::is_zig(&named.program)
        {
            return Ok(zig::driver(&named.program, &named.args));
        }
        #[cfg(windows)]
        if named.is_none()
            && let Some(compiler) = windows::without_msvc(cpp)?
        {
            return Ok(compiler);
        }
        Self::through_cc(cpp)
    }

    /// The driver `cc` chooses, with the flags it would compile with.
    fn through_cc(cpp: bool) -> Result<Self, CompileError> {
        let tool = cc::Build::new()
            .cpp(cpp)
            .cargo_metadata(false)
            .cargo_warnings(false)
            .debug(false)
            .opt_level(2)
            .warnings(false)
            .host(TARGET)
            .target(TARGET)
            .try_get_compiler()
            .map_err(|e| CompileError::NoCompiler(e.to_string()))?;
        // `to_command` is what adds the extra words of `CC`/`CXX` (a wrapper
        // such as `ccache`, or flags), which `path()` and `args()` leave out.
        let command = tool.to_command();
        Ok(Self {
            program: command.get_program().into(),
            args: command.get_args().map(OsString::from).collect(),
            env: command
                .get_envs()
                .map(|(key, value)| (key.to_owned(), value.map(OsString::from)))
                .collect(),
            flavor: if tool.is_like_msvc() {
                Flavor::Msvc
            } else {
                Flavor::Gnu
            },
        })
    }

    pub(crate) fn flavor(&self) -> Flavor {
        self.flavor
    }

    /// Run the driver with `args` after its own, in `cwd`. What it prints
    /// is kept for the error, not shown, since builds run in parallel.
    pub(crate) fn run(&self, args: &[OsString], cwd: &Path) -> Result<(), CompileError> {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::null());
        for (key, value) in &self.env {
            match value {
                Some(value) => command.env(key, value),
                None => command.env_remove(key),
            };
        }
        let output = command.output().map_err(|source| CompileError::Spawn {
            program: self.program.clone(),
            source,
        })?;
        if output.status.success() {
            Ok(())
        } else {
            let mut printed = String::from_utf8_lossy(&output.stdout).into_owned();
            printed.push_str(&String::from_utf8_lossy(&output.stderr));
            Err(CompileError::Failed {
                program: self.program.clone(),
                status: output.status,
                output: printed,
            })
        }
    }
}

/// A compiler named in the environment: its program and the words after it.
#[derive(Debug, PartialEq, Eq)]
struct EnvCompiler {
    program: String,
    args: Vec<String>,
}

/// The compiler `CC`, or `CXX` when `cpp`, names, read as `cc` reads it.
///
/// The first of `<VAR>_<target>`, `<VAR>_<target_with_underscores>`,
/// `HOST_<VAR>` and `<VAR>` that is set wins, and an empty one names no
/// compiler (grammars are built for the host, so `cc` never reads
/// `TARGET_<VAR>`). A value that is an existing path is one word, so it may
/// hold spaces; any other value splits on whitespace.
fn env_compiler(cpp: bool, getenv: impl Fn(&str) -> Option<OsString>) -> Option<EnvCompiler> {
    let var = if cpp { "CXX" } else { "CC" };
    let names = [
        format!("{var}_{TARGET}"),
        format!("{var}_{}", TARGET.replace(['-', '.'], "_")),
        format!("HOST_{var}"),
        var.to_string(),
    ];
    let value = names.iter().find_map(|name| getenv(name))?;
    let value = value.to_string_lossy();
    let value = value.trim();
    if exists(value) {
        return Some(EnvCompiler {
            program: value.to_string(),
            args: Vec::new(),
        });
    }
    let mut words = value.split_whitespace().map(String::from);
    Some(EnvCompiler {
        program: words.next()?,
        args: words.collect(),
    })
}

/// Whether `path` exists, with or without the platform's executable
/// extension.
fn exists(path: &str) -> bool {
    let path = Path::new(path);
    path.exists()
        || (!std::env::consts::EXE_EXTENSION.is_empty()
            && path
                .with_added_extension(std::env::consts::EXE_EXTENSION)
                .exists())
}

#[cfg(test)]
mod tests;
#[cfg(windows)]
mod windows;
mod zig;

use std::path::{Path, PathBuf};

use crate::compile::CompileError;

/// A grammar's sources, as `tree-sitter generate` lays them out in `src/`.
#[derive(Debug)]
pub(crate) struct Sources {
    /// The include root for `tree_sitter/parser.h`.
    pub(crate) dir: PathBuf,
    pub(crate) parser: PathBuf,
    pub(crate) scanner: Option<Scanner>,
}

/// An external scanner, by the language it is written in.
#[derive(Debug)]
pub(crate) enum Scanner {
    C(PathBuf),
    Cpp(PathBuf),
}

impl Sources {
    /// The scanner is the first of `scanner.c`, `scanner.cc` and
    /// `scanner.cpp` that the grammar ships.
    pub(crate) fn find(grammar_dir: &Path) -> Result<Self, CompileError> {
        let dir = grammar_dir.join("src");
        let parser = dir.join("parser.c");
        if !parser.is_file() {
            return Err(CompileError::NoParser(parser));
        }
        let scanner = [
            ("scanner.c", Scanner::C as fn(PathBuf) -> Scanner),
            ("scanner.cc", Scanner::Cpp),
            ("scanner.cpp", Scanner::Cpp),
        ]
        .into_iter()
        .map(|(name, scanner)| (dir.join(name), scanner))
        .find(|(path, _)| path.is_file())
        .map(|(path, scanner)| scanner(path));
        Ok(Self {
            dir,
            parser,
            scanner,
        })
    }
}

#[cfg(test)]
mod tests;

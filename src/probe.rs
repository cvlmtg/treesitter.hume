//! Loading a built grammar the way HUME loads it, to read its ABI, to
//! prove its symbol resolves on the platform it was built for, and to prove
//! the queries shipped with it compile against it.

use std::fmt;
use std::ops::RangeInclusive;
use std::path::Path;

use tree_sitter::{LANGUAGE_VERSION, Language, MIN_COMPATIBLE_LANGUAGE_VERSION, Query, QueryError};
use tree_sitter_language::LanguageFn;

/// The ABIs the tree-sitter runtime this tool links can load, the same
/// release HUME links.
const SUPPORTED: RangeInclusive<usize> = MIN_COMPATIBLE_LANGUAGE_VERSION..=LANGUAGE_VERSION;

#[derive(Debug)]
pub(crate) enum ProbeError {
    Load(libloading::Error),
    Symbol {
        symbol: String,
        source: libloading::Error,
    },
    Abi(usize),
    Query {
        file: String,
        source: QueryError,
    },
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(e) => write!(f, "cannot load the library: {e}"),
            Self::Symbol { symbol, source } => write!(f, "no symbol {symbol}: {source}"),
            Self::Abi(abi) => write!(f, "ABI {abi} is outside the supported {SUPPORTED:?}"),
            Self::Query { file, source } => write!(f, "queries/{file}: {source}"),
        }
    }
}

/// The ABI of the grammar `symbol` returns from `library`, which must be
/// one tree-sitter can load, and against which each of `queries`, given as
/// `(file name, text)`, must compile.
pub(crate) fn check(
    library: &Path,
    symbol: &str,
    queries: &[(String, String)],
) -> Result<usize, ProbeError> {
    // SAFETY: the library is a grammar this tool just built, and `symbol`
    // is its language function as tree-sitter generates it: it takes
    // nothing and returns a pointer to a static `TSLanguage`.
    unsafe {
        let library = libloading::Library::new(library).map_err(ProbeError::Load)?;
        let function = library
            .get::<unsafe extern "C" fn() -> *const ()>(symbol)
            .map_err(|source| ProbeError::Symbol {
                symbol: symbol.to_string(),
                source,
            })?;
        let language = Language::from(LanguageFn::from_raw(*function));
        // The language and each query point into the library, so they go
        // first.
        let checked = check_language(&language, queries);
        drop(language);
        checked
    }
}

fn check_language(language: &Language, queries: &[(String, String)]) -> Result<usize, ProbeError> {
    let abi = language.abi_version();
    if !SUPPORTED.contains(&abi) {
        return Err(ProbeError::Abi(abi));
    }
    for (file, text) in queries {
        Query::new(language, text).map_err(|source| ProbeError::Query {
            file: file.clone(),
            source,
        })?;
    }
    Ok(abi)
}

#[cfg(test)]
mod tests;

//! Loading a built grammar the way HUME loads it, to read its ABI and to
//! prove its symbol resolves on the platform it was built for.

use std::fmt;
use std::ops::RangeInclusive;
use std::path::Path;

use tree_sitter::{LANGUAGE_VERSION, Language, MIN_COMPATIBLE_LANGUAGE_VERSION};
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
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(e) => write!(f, "cannot load the library: {e}"),
            Self::Symbol { symbol, source } => write!(f, "no symbol {symbol}: {source}"),
            Self::Abi(abi) => write!(f, "ABI {abi} is outside the supported {SUPPORTED:?}"),
        }
    }
}

/// The ABI of the grammar `symbol` returns from `library`, which must be
/// one tree-sitter can load.
pub(crate) fn abi(library: &Path, symbol: &str) -> Result<usize, ProbeError> {
    // SAFETY: the library is a grammar this tool just built, and `symbol`
    // is its language function as tree-sitter generates it: it takes
    // nothing and returns a pointer to a static `TSLanguage`.
    let abi = unsafe {
        let library = libloading::Library::new(library).map_err(ProbeError::Load)?;
        let function = library
            .get::<unsafe extern "C" fn() -> *const ()>(symbol)
            .map_err(|source| ProbeError::Symbol {
                symbol: symbol.to_string(),
                source,
            })?;
        let language = Language::from(LanguageFn::from_raw(*function));
        // The language points into the library, so it goes first.
        let abi = language.abi_version();
        drop(language);
        abi
    };
    if SUPPORTED.contains(&abi) {
        Ok(abi)
    } else {
        Err(ProbeError::Abi(abi))
    }
}

#[cfg(test)]
mod tests;

//! Helix's query files, with `; inherits:` resolved the way Helix resolves
//! it (tree-house 0.4.0's `read_query`): each directive, wherever it
//! appears, gives way to the named languages' files of the same name,
//! themselves resolved, each framed by newlines.

use std::fmt;
use std::io;
use std::path::Path;
use std::sync::LazyLock;

use regex::{Captures, Regex};

static INHERITS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r";+\s*inherits\s*:?\s*([a-z_,()-]+)\s*").expect("the inherits pattern is valid")
});

/// A query file with its inheritance resolved.
#[derive(Debug)]
pub(crate) struct Resolved {
    pub(crate) text: String,
    /// The parents named but with no file, which resolve to nothing as in
    /// Helix; a typo here drops a parent's queries.
    pub(crate) missing: Vec<String>,
}

#[derive(Debug)]
pub(crate) enum QueryError {
    Missing(String),
    Cycle(Vec<String>),
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(language) => write!(f, "no query file for {language}"),
            Self::Cycle(chain) => write!(f, "query inheritance cycle: {}", chain.join(" -> ")),
        }
    }
}

/// `language`'s query file, with every parent it inherits resolved.
/// `read` returns a language's file, or `None` when it has none.
pub(crate) fn resolve(
    language: &str,
    read: impl Fn(&str) -> Option<String>,
) -> Result<Resolved, QueryError> {
    let text = read(language).ok_or_else(|| QueryError::Missing(language.to_string()))?;
    let mut missing = Vec::new();
    let text = expand(&text, &read, &mut vec![language.to_string()], &mut missing)?;
    Ok(Resolved { text, missing })
}

/// `text` with each directive replaced by its parents. `chain` holds the
/// languages being resolved, outermost first.
fn expand(
    text: &str,
    read: &impl Fn(&str) -> Option<String>,
    chain: &mut Vec<String>,
    missing: &mut Vec<String>,
) -> Result<String, QueryError> {
    let mut error = None;
    let expanded = INHERITS.replace_all(text, |captures: &Captures| {
        let mut parents = String::new();
        for parent in captures[1].split(',') {
            match parent_text(parent, read, chain, missing) {
                Ok(text) => {
                    parents.push('\n');
                    parents.push_str(&text);
                    parents.push('\n');
                }
                Err(e) => {
                    error.get_or_insert(e);
                }
            }
        }
        parents
    });
    match error {
        Some(e) => Err(e),
        None => Ok(expanded.into_owned()),
    }
}

fn parent_text(
    parent: &str,
    read: &impl Fn(&str) -> Option<String>,
    chain: &mut Vec<String>,
    missing: &mut Vec<String>,
) -> Result<String, QueryError> {
    if chain.iter().any(|language| language == parent) {
        let mut cycle = chain.clone();
        cycle.push(parent.to_string());
        return Err(QueryError::Cycle(cycle));
    }
    // An empty name comes from a list such as `a,`; Helix finds no file for it.
    let Some(text) = (!parent.is_empty()).then(|| read(parent)).flatten() else {
        missing.push(parent.to_string());
        return Ok(String::new());
    };
    chain.push(parent.to_string());
    let expanded = expand(&text, read, chain, missing);
    chain.pop();
    expanded
}

/// The names of the query files in `dir`, in name order.
pub(crate) fn query_files(dir: &Path) -> io::Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && let Some(name) = entry.file_name().to_str()
            && name.ends_with(".scm")
        {
            names.push(name.to_string());
        }
    }
    names.sort();
    Ok(names)
}

#[cfg(test)]
mod tests;

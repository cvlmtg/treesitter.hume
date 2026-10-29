//! The notices a language archive carries for what it redistributes: its
//! grammar's license files with a note on where the grammar's source is,
//! and the MPL-2.0 notice of Helix's query files.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::pins::Pin;

static LICENSE_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(LICEN[CS]E|COPYING|NOTICE|UNLICENSE)([.-].*)?$")
        .expect("the license name pattern is valid")
});

#[derive(Debug)]
pub(crate) enum LicenseError {
    /// `searched` names the directories looked in.
    None {
        searched: String,
        rev: String,
    },
    Io {
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for LicenseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::None { searched, rev } => write!(f, "no license file in {searched} at {rev}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

/// The license files of `pin`'s grammar in its `checkout`, from the
/// repository root and from the grammar's subpath, each as
/// `licenses/<repository path>`, and `licenses/SOURCE` naming the commit
/// the library is compiled from. A grammar with no license file grants no
/// right to redistribute it, so it is an error.
pub(crate) fn grammar(pin: &Pin, checkout: &Path) -> Result<Vec<(String, Vec<u8>)>, LicenseError> {
    let mut dirs = vec![""];
    if !pin.subpath.is_empty() {
        dirs.push(&pin.subpath);
    }
    let mut files = Vec::new();
    for dir in &dirs {
        let path = checkout.join(dir);
        let entries = fs::read_dir(&path).map_err(|source| LicenseError::Io {
            path: path.clone(),
            source,
        })?;
        for entry in entries {
            let entry = entry.map_err(|source| LicenseError::Io {
                path: path.clone(),
                source,
            })?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if !is_license_name(&name) || !entry.path().is_file() {
                continue;
            }
            let contents = fs::read(entry.path()).map_err(|source| LicenseError::Io {
                path: entry.path(),
                source,
            })?;
            let archived = if dir.is_empty() {
                format!("licenses/{name}")
            } else {
                format!("licenses/{dir}/{name}")
            };
            files.push((archived, contents));
        }
    }
    if files.is_empty() {
        let searched = if pin.subpath.is_empty() {
            "the repository root".to_string()
        } else {
            format!("the repository root or {}", pin.subpath)
        };
        return Err(LicenseError::None {
            searched,
            rev: pin.rev.clone(),
        });
    }
    files.push(("licenses/SOURCE".to_string(), source_note(pin).into_bytes()));
    files.sort();
    Ok(files)
}

fn is_license_name(name: &str) -> bool {
    LICENSE_NAME.is_match(name)
}

fn source_note(pin: &Pin) -> String {
    let directory = if pin.subpath.is_empty() {
        String::new()
    } else {
        format!(", directory {}", pin.subpath)
    };
    format!(
        "The grammar library in this archive is compiled from\n\
         {url} at commit {rev}{directory}.\n\
         That is its source code. The license files beside this one come from\n\
         the same commit.\n",
        url = pin.url,
        rev = pin.rev,
    )
}

/// The text of `queries/NOTICE` for `language`'s queries, taken from Helix
/// at `helix_url` and `helix_rev`.
pub(crate) fn queries_notice(language: &str, helix_url: &str, helix_rev: &str) -> String {
    format!(
        "These query files are Helix's runtime/queries/{language}/ files, from\n\
         {helix_url} at commit {helix_rev}.\n\
         Each `; inherits:` line is replaced by the queries of the languages it\n\
         names.\n\
         \n\
         They are covered by the Mozilla Public License 2.0, which you can get\n\
         at https://mozilla.org/MPL/2.0/. Their source is at the commit above.\n"
    )
}

#[cfg(test)]
mod tests;

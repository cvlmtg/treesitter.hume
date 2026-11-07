//! The notices a language archive carries for what it redistributes: its
//! grammar's license files with a note on where the grammar's source is,
//! and the notice of its query files: where they come from and their
//! licenses.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;

use crate::languages::{Pin, Upstream};

const HELIX_URL: &str = "https://github.com/helix-editor/helix";
const CONTEXT_URL: &str = "https://github.com/nvim-treesitter/nvim-treesitter-context";
const REPO_URL: &str = "https://github.com/cvlmtg/treesitter.hume";
const CONTEXT_LICENSE: &str = include_str!("../notices/nvim-treesitter-context.LICENSE");

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

/// The text of `queries/NOTICE` for `language`'s queries, which this
/// repo's `tag` ships with the changes it makes to them.
pub(crate) fn queries_notice(language: &str, upstream: &Upstream, tag: &str) -> String {
    let helix = &upstream.helix;
    let origin = match &upstream.context {
        Some(context) => format!(
            "context.scm comes from nvim-treesitter-context, {CONTEXT_URL},\n\
             at commit {context}. The other query files come from Helix's\n\
             runtime/queries, {HELIX_URL}, at commit {helix}.\n"
        ),
        None => format!(
            "These query files come from Helix's runtime/queries,\n\
             {HELIX_URL}, at commit {helix}.\n"
        ),
    };
    let mut notice = format!(
        "{origin}\
         treesitter.hume may change them. The files as shipped are in\n\
         {REPO_URL} at tag {tag}, in languages/{language}/ and\n\
         the folders their `; inherits:` lines name. Each `; inherits:` line is\n\
         replaced by the queries of the languages it names.\n\
         \n\
         The files from Helix are covered by the Mozilla Public License 2.0,\n\
         which you can get at https://mozilla.org/MPL/2.0/.\n"
    );
    if upstream.context.is_some() {
        notice.push_str("\ncontext.scm is covered by this license:\n\n");
        notice.push_str(CONTEXT_LICENSE);
    }
    notice
}

#[cfg(test)]
mod tests;

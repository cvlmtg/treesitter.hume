//! The languages this repo builds: one folder per language under
//! `languages/`, and `exclusions.toml` (kept by hand), compiled into the
//! binary so a build of version `v` always uses `v`'s pins and queries.
//!
//! A folder's `source.toml` names the upstream commits its files come from,
//! or, in its `[own]` table, the files this repo wrote, and, for a language
//! with a grammar, pins that grammar. A folder without a pin holds queries
//! other languages inherit. `build.rs` embeds each folder's `source.toml`
//! and query files.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::Deserialize;

const EXCLUSIONS: &str = include_str!("../exclusions.toml");

static EMBEDDED: &[Folder<'static>] = &include!(concat!(env!("OUT_DIR"), "/languages.rs"));

/// The longest language name whose archive, `<name>.tar.gz`, still fits the
/// 100 bytes a ustar header gives a member's name.
const MAX_NAME_LEN: usize = 100 - ".tar.gz".len();

const CONTEXT_FILE: &str = "context.scm";

/// One folder of `languages/`, as text.
#[derive(Clone, Copy)]
pub(crate) struct Folder<'a> {
    pub(crate) name: &'a str,
    /// The folder's `source.toml`.
    pub(crate) source: &'a str,
    /// Each query file's name and text.
    pub(crate) queries: &'a [(&'a str, &'a str)],
}

/// Where a folder's query files come from: Helix's commit for every file
/// but `context.scm`. `context` is set if and only if the folder has a
/// `context.scm`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Origin {
    pub(crate) helix: String,
    pub(crate) context: Option<ContextOrigin>,
}

/// Where `context.scm` comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ContextOrigin {
    /// The nvim-treesitter-context commit.
    Upstream(String),
    /// Written in this repo.
    Own,
}

/// The upstream commits a folder's files come from: Helix for every file
/// but `context.scm`, nvim-treesitter-context for `context.scm`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Upstream {
    helix: String,
    context: Option<String>,
}

/// The files this repo wrote instead of taking them from upstream.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Own {
    context: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceFile {
    upstream: Upstream,
    own: Option<Own>,
    grammar: Option<Grammar>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Grammar {
    url: String,
    rev: String,
    subpath: String,
    symbol: String,
}

/// One language's grammar: where its source is, and the C function that
/// returns it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pin {
    pub(crate) name: String,
    pub(crate) url: String,
    /// A full commit SHA.
    pub(crate) rev: String,
    /// The directory holding the grammar's `src/` within its repository,
    /// empty for the root.
    pub(crate) subpath: String,
    pub(crate) symbol: String,
    /// Where the language's own query files come from.
    pub(crate) origin: Origin,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExclusionsFile {
    #[serde(default)]
    exclude: Vec<Exclusion>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Exclusion {
    name: String,
    /// Empty for every platform.
    #[serde(default)]
    platforms: Vec<String>,
    reason: String,
}

/// Every pinned language, sorted by name, and every folder's query files.
pub(crate) struct Languages<'a> {
    pins: Vec<Pin>,
    queries: BTreeMap<&'a str, BTreeMap<&'a str, &'a str>>,
    exclusions: Vec<Exclusion>,
}

/// Why the languages are unusable.
#[derive(Debug)]
pub(crate) struct LanguagesError(String);

impl fmt::Display for LanguagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Languages<'static> {
    pub(crate) fn embedded() -> Result<Self, LanguagesError> {
        Self::parse(EMBEDDED.iter().copied(), EXCLUSIONS)
    }
}

impl<'a> Languages<'a> {
    pub(crate) fn parse(
        folders: impl IntoIterator<Item = Folder<'a>>,
        exclusions: &str,
    ) -> Result<Self, LanguagesError> {
        let mut pins = Vec::new();
        let mut queries = BTreeMap::new();
        for folder in folders {
            if queries.contains_key(folder.name) {
                return Err(LanguagesError(format!(
                    "folder {:?} appears twice",
                    folder.name
                )));
            }
            let files: BTreeMap<&str, &str> = folder.queries.iter().copied().collect();
            if let Some(pin) = parse_folder(&folder, &files)? {
                pins.push(pin);
            }
            queries.insert(folder.name, files);
        }
        pins.sort_by(|a, b| a.name.cmp(&b.name));

        let ExclusionsFile { exclude } = toml::from_str(exclusions)
            .map_err(|e| LanguagesError(format!("exclusions.toml: {e}")))?;
        let pinned: BTreeSet<&str> = pins.iter().map(|pin| pin.name.as_str()).collect();
        for exclusion in &exclude {
            if !pinned.contains(exclusion.name.as_str()) {
                return Err(LanguagesError(format!(
                    "exclusion of {:?}, which is not pinned",
                    exclusion.name
                )));
            }
            if exclusion.reason.trim().is_empty() {
                return Err(LanguagesError(format!(
                    "exclusion of {:?} gives no reason",
                    exclusion.name
                )));
            }
        }
        Ok(Self {
            pins,
            queries,
            exclusions: exclude,
        })
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Pin> {
        self.pins
            .binary_search_by(|pin| pin.name.as_str().cmp(name))
            .ok()
            .map(|i| &self.pins[i])
    }

    /// The names of `folder`'s query files, in name order.
    pub(crate) fn query_files(&self, folder: &str) -> Vec<&'a str> {
        self.queries
            .get(folder)
            .map(|files| files.keys().copied().collect())
            .unwrap_or_default()
    }

    /// The text of `folder`'s query file `file`, if it has one.
    pub(crate) fn query(&self, folder: &str, file: &str) -> Option<&'a str> {
        self.queries.get(folder)?.get(file).copied()
    }

    /// The languages a release for `platform` holds: every pin its
    /// exclusions leave in.
    pub(crate) fn release(&self, platform: &str) -> Vec<&Pin> {
        self.pins
            .iter()
            .filter(|pin| {
                !self.exclusions.iter().any(|exclusion| {
                    exclusion.name == pin.name
                        && (exclusion.platforms.is_empty()
                            || exclusion.platforms.iter().any(|p| p == platform))
                })
            })
            .collect()
    }
}

/// `folder`'s pin, or None for a folder of inherited queries only.
fn parse_folder(
    folder: &Folder,
    files: &BTreeMap<&str, &str>,
) -> Result<Option<Pin>, LanguagesError> {
    let name = folder.name;
    if !is_language_name(name) {
        return Err(LanguagesError(format!(
            "folder name {name:?} must be lowercase ASCII letters, digits, '.', '_' or '-', \
             not start with '.', and be at most {MAX_NAME_LEN} bytes"
        )));
    }
    let SourceFile {
        upstream,
        own,
        grammar,
    } = toml::from_str(folder.source)
        .map_err(|e| LanguagesError(format!("{name}/source.toml: {e}")))?;
    check_rev(&format!("{name}: upstream helix"), &upstream.helix)?;
    let own_context = match own {
        Some(Own { context: false }) => {
            return Err(LanguagesError(format!(
                "{name}: an [own] table that names no file"
            )));
        }
        Some(Own { context: true }) => true,
        None => false,
    };
    let context = match (upstream.context, own_context) {
        (Some(_), true) => {
            return Err(LanguagesError(format!(
                "{name}: an upstream context commit, and {CONTEXT_FILE} in [own]"
            )));
        }
        (Some(rev), false) => {
            check_rev(&format!("{name}: upstream context"), &rev)?;
            Some(ContextOrigin::Upstream(rev))
        }
        (None, true) => Some(ContextOrigin::Own),
        (None, false) => None,
    };
    match (&context, files.contains_key(CONTEXT_FILE)) {
        (Some(_), false) => {
            return Err(LanguagesError(format!(
                "{name}: a {CONTEXT_FILE} origin, but no {CONTEXT_FILE}"
            )));
        }
        (None, true) => {
            return Err(LanguagesError(format!(
                "{name}: {CONTEXT_FILE}, but neither an upstream context commit nor [own] context"
            )));
        }
        _ => {}
    }
    let Some(grammar) = grammar else {
        return Ok(None);
    };
    check_rev(name, &grammar.rev)?;
    if !is_c_identifier(&grammar.symbol) {
        return Err(LanguagesError(format!(
            "{name}: symbol {:?} is not a C identifier",
            grammar.symbol
        )));
    }
    if !files.contains_key("highlights.scm") {
        return Err(LanguagesError(format!(
            "{name}: pinned, but has no highlights.scm"
        )));
    }
    Ok(Some(Pin {
        name: name.to_string(),
        url: grammar.url,
        rev: grammar.rev,
        subpath: grammar.subpath,
        symbol: grammar.symbol,
        origin: Origin {
            helix: upstream.helix,
            context,
        },
    }))
}

fn check_rev(what: &str, rev: &str) -> Result<(), LanguagesError> {
    if rev.len() == 40 && rev.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        Ok(())
    } else {
        Err(LanguagesError(format!(
            "{what}: rev {rev:?} is not a full commit SHA"
        )))
    }
}

/// Lowercase so names stay distinct on case-insensitive file systems, and
/// no leading dot so an archive is never a dotfile.
fn is_language_name(name: &str) -> bool {
    let allowed = |b: u8| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'.' | b'_' | b'-');
    name.len() <= MAX_NAME_LEN
        && name.bytes().all(allowed)
        && name.bytes().next().is_some_and(|b| b != b'.')
}

fn is_c_identifier(symbol: &str) -> bool {
    symbol
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && symbol
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

#[cfg(test)]
mod tests;

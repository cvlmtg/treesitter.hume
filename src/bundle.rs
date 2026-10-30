//! A build writes one `<language>.tar.gz` per language into a folder, which
//! HUME installs from as it is. A release packs that folder into a
//! platform's bundle: `grammars-<platform>.tar`, an uncompressed tar of the
//! language archives, and `grammars-<platform>.index`, which says where each
//! one starts.
//!
//! The index is text. Its first line is `treesitter.hume <tag> <platform>`;
//! each other line is `<language> <offset> <size> <sha256>`, so a client
//! fetches one language with a single ranged read of the tar and checks it
//! against the digest.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use crate::manifest;
use crate::sums::sha256_hex;

const ARCHIVE_SUFFIX: &str = ".tar.gz";

/// A ustar header occupies one 512-byte block, and member data is padded to
/// a whole number of blocks.
const BLOCK: usize = 512;

/// One language's archive: a deterministic `.tar.gz` of `files`, given as
/// `(path, contents)`, in path order with no timestamps or owners.
pub(crate) fn archive(files: &[(String, Vec<u8>)]) -> io::Result<Vec<u8>> {
    let mut files: Vec<&(String, Vec<u8>)> = files.iter().collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::best()));
    for (path, contents) in files {
        tar.append_data(&mut header(contents.len()), path, contents.as_slice())?;
    }
    tar.into_inner()?.finish()
}

/// Where `language`'s archive sits in a build's folder.
fn archive_path(dir: &Path, language: &str) -> PathBuf {
    dir.join(format!("{language}{ARCHIVE_SUFFIX}"))
}

/// Write `language`'s archive into the folder `dir`, replacing an earlier
/// one.
pub(crate) fn write_archive(dir: &Path, language: &str, archive: &[u8]) -> io::Result<()> {
    replace(&archive_path(dir, language), archive)
}

fn header(size: usize) -> tar::Header {
    let mut header = tar::Header::new_ustar();
    header.set_size(size as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_uid(0);
    header.set_gid(0);
    header
}

#[derive(Debug)]
pub(crate) enum BundleError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    /// An archive was built for another language, platform or release.
    Foreign(String),
    /// The folder holds no language archive.
    Empty(PathBuf),
    Corrupt {
        path: PathBuf,
        problem: String,
    },
}

impl fmt::Display for BundleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Foreign(problem) => f.write_str(problem),
            Self::Empty(dir) => write!(
                f,
                "{}: no <language>{ARCHIVE_SUFFIX} to bundle",
                dir.display()
            ),
            Self::Corrupt { path, problem } => write!(f, "{}: {problem}", path.display()),
        }
    }
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> BundleError {
    let path = path.to_path_buf();
    move |source| BundleError::Io { path, source }
}

/// A platform's bundle of language archives, keyed by language.
#[derive(Debug)]
pub(crate) struct Bundle {
    tag: String,
    platform: String,
    members: BTreeMap<String, Vec<u8>>,
}

impl Bundle {
    fn new(tag: &str, platform: &str) -> Self {
        Self {
            tag: tag.to_string(),
            platform: platform.to_string(),
            members: BTreeMap::new(),
        }
    }

    /// The bundle of every language archive in the folder `grammars`. An
    /// archive whose manifest names another language than its file name, or
    /// another tag or platform, is refused, so one bundle never mixes
    /// archives from two releases.
    pub(crate) fn read(grammars: &Path, tag: &str, platform: &str) -> Result<Self, BundleError> {
        let mut bundle = Self::new(tag, platform);
        for entry in fs::read_dir(grammars).map_err(io_error(grammars))? {
            let path = entry.map_err(io_error(grammars))?.path();
            let Some(language) = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_suffix(ARCHIVE_SUFFIX))
            else {
                continue;
            };
            let archive = fs::read(&path).map_err(io_error(&path))?;
            bundle.check(&path, language, &archive)?;
            bundle.insert(language, archive);
        }
        if bundle.members.is_empty() {
            return Err(BundleError::Empty(grammars.to_path_buf()));
        }
        Ok(bundle)
    }

    /// Refuses `archive`, read from `path`, unless its manifest names
    /// `language` and this bundle's tag and platform.
    fn check(&self, path: &Path, language: &str, archive: &[u8]) -> Result<(), BundleError> {
        let text = manifest_text(archive).map_err(|problem| BundleError::Corrupt {
            path: path.to_path_buf(),
            problem,
        })?;
        for (key, expected) in [
            ("name", language),
            ("platform", self.platform.as_str()),
            ("tag", self.tag.as_str()),
        ] {
            let found = manifest::read_string(&text, key);
            if found.as_deref() != Some(expected) {
                return Err(BundleError::Foreign(format!(
                    "{}: its manifest's {key} is {found:?}, not \"{expected}\"",
                    path.display()
                )));
            }
        }
        Ok(())
    }

    /// Add or replace `language`'s archive.
    fn insert(&mut self, language: &str, archive: Vec<u8>) {
        self.members.insert(language.to_string(), archive);
    }

    /// Write the tar and its index into `dir`, each replacing any previous
    /// one whole; the index goes last, so it never names a member the tar
    /// does not hold.
    pub(crate) fn write(&self, dir: &Path) -> Result<(), BundleError> {
        fs::create_dir_all(dir).map_err(io_error(dir))?;
        let tar_path = self.tar_path(dir);
        let mut tar = tar::Builder::new(Vec::new());
        let mut index = self.index_header();
        for (language, archive) in &self.members {
            let name = format!("{language}.tar.gz");
            let offset = tar.get_ref().len() + BLOCK;
            tar.append_data(&mut header(archive.len()), &name, archive.as_slice())
                .map_err(io_error(&tar_path))?;
            let padded = archive.len().div_ceil(BLOCK) * BLOCK;
            assert_eq!(
                tar.get_ref().len(),
                offset + padded,
                "{name} took more than one header block"
            );
            index.push_str(&format!(
                "\n{language} {offset} {} {}",
                archive.len(),
                sha256_hex(archive)
            ));
        }
        index.push('\n');
        let tar = tar.into_inner().map_err(io_error(&tar_path))?;
        replace(&tar_path, &tar).map_err(io_error(&tar_path))?;
        let index_path = self.index_path(dir);
        replace(&index_path, index.as_bytes()).map_err(io_error(&index_path))
    }

    /// How many language archives the bundle holds.
    pub(crate) fn len(&self) -> usize {
        self.members.len()
    }

    fn index_header(&self) -> String {
        format!("treesitter.hume {} {}", self.tag, self.platform)
    }

    fn tar_path(&self, dir: &Path) -> PathBuf {
        dir.join(format!("grammars-{}.tar", self.platform))
    }

    fn index_path(&self, dir: &Path) -> PathBuf {
        dir.join(format!("grammars-{}.index", self.platform))
    }
}

/// The text of `archive`'s `manifest.scm`.
fn manifest_text(archive: &[u8]) -> Result<String, String> {
    let mut tar = tar::Archive::new(GzDecoder::new(archive));
    for entry in tar.entries().map_err(|e| e.to_string())? {
        let mut entry = entry.map_err(|e| e.to_string())?;
        if entry.path().map_err(|e| e.to_string())?.as_os_str() == "manifest.scm" {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(|e| format!("manifest.scm: {e}"))?;
            return Ok(text);
        }
    }
    Err("no manifest.scm".to_string())
}

/// Write `path` through a dotfile beside it, so it is never seen half
/// written.
fn replace(path: &Path, contents: &[u8]) -> io::Result<()> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("bundle paths end in a UTF-8 file name");
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    let mut file = fs::File::create(&tmp)?;
    file.write_all(contents)?;
    file.sync_all()?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests;

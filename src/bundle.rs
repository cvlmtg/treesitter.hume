//! A platform's release: `grammars-<platform>.tar`, an uncompressed tar of
//! one `<language>.tar.gz` per language, and `grammars-<platform>.index`,
//! which says where each one starts.
//!
//! The index is text. Its first line is `treesitter.hume <tag> <platform>`;
//! each other line is `<language> <offset> <size> <sha256>`, so a client
//! fetches one language with a single ranged read of the tar and checks it
//! against the digest.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::write::GzEncoder;

use crate::sums::sha256_hex;

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
    /// The output directory holds a bundle the tool cannot extend.
    Foreign(String),
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
    pub(crate) fn new(tag: &str, platform: &str) -> Self {
        Self {
            tag: tag.to_string(),
            platform: platform.to_string(),
            members: BTreeMap::new(),
        }
    }

    /// The bundle for `tag` and `platform` in `dir`, or an empty one when
    /// `dir` holds none. A bundle from another tag or platform is refused,
    /// so one bundle never mixes archives from two releases.
    pub(crate) fn open(dir: &Path, tag: &str, platform: &str) -> Result<Self, BundleError> {
        let mut bundle = Self::new(tag, platform);
        let (tar_path, index_path) = (bundle.tar_path(dir), bundle.index_path(dir));
        match (tar_path.exists(), index_path.exists()) {
            (false, false) => return Ok(bundle),
            (true, true) => {}
            _ => {
                return Err(BundleError::Foreign(format!(
                    "{} needs both {} and {}; remove the one left",
                    dir.display(),
                    tar_path.display(),
                    index_path.display()
                )));
            }
        }
        let index = fs::read_to_string(&index_path).map_err(io_error(&index_path))?;
        let tar = fs::read(&tar_path).map_err(io_error(&tar_path))?;
        let corrupt = |problem: String| BundleError::Corrupt {
            path: index_path.clone(),
            problem,
        };
        let mut lines = index.lines();
        let expected = bundle.index_header();
        match lines.next() {
            Some(header) if header == expected => {}
            Some(header) => {
                return Err(BundleError::Foreign(format!(
                    "{} is \"{header}\", not \"{expected}\"; build into an empty directory",
                    index_path.display()
                )));
            }
            None => return Err(corrupt("empty index".to_string())),
        }
        for line in lines {
            let [language, offset, size, digest] = line.split(' ').collect::<Vec<_>>()[..] else {
                return Err(corrupt(format!("malformed line {line:?}")));
            };
            let range = offset
                .parse::<usize>()
                .ok()
                .zip(size.parse::<usize>().ok())
                .and_then(|(offset, size)| Some(offset..offset.checked_add(size)?))
                .filter(|range| range.end <= tar.len())
                .ok_or_else(|| corrupt(format!("{language}: no such range of the tar")))?;
            let bytes = &tar[range];
            if sha256_hex(bytes) != digest {
                return Err(corrupt(format!(
                    "{language}: the tar does not match its digest"
                )));
            }
            bundle.members.insert(language.to_string(), bytes.to_vec());
        }
        Ok(bundle)
    }

    /// Add or replace `language`'s archive.
    pub(crate) fn insert(&mut self, language: &str, archive: Vec<u8>) {
        self.members.insert(language.to_string(), archive);
    }

    /// Write the tar and its index into `dir`, each replacing any previous
    /// one whole; the index goes last, so it never names a member the tar
    /// does not hold.
    pub(crate) fn write(&self, dir: &Path) -> Result<(), BundleError> {
        let mut tar = tar::Builder::new(Vec::new());
        let mut index = self.index_header();
        for (language, archive) in &self.members {
            let name = format!("{language}.tar.gz");
            let offset = tar.get_ref().len() + BLOCK;
            tar.append_data(&mut header(archive.len()), &name, archive.as_slice())
                .map_err(io_error(&self.tar_path(dir)))?;
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
        let tar = tar.into_inner().map_err(io_error(&self.tar_path(dir)))?;
        replace(&self.tar_path(dir), &tar)?;
        replace(&self.index_path(dir), index.as_bytes())
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

/// Write `path` through a dotfile beside it, so it is never seen half
/// written.
fn replace(path: &Path, contents: &[u8]) -> Result<(), BundleError> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("bundle paths end in a UTF-8 file name");
    let tmp = path.with_file_name(format!(".{name}.tmp"));
    let mut file = fs::File::create(&tmp).map_err(io_error(&tmp))?;
    file.write_all(contents).map_err(io_error(&tmp))?;
    file.sync_all().map_err(io_error(&tmp))?;
    fs::rename(&tmp, path).map_err(io_error(path))
}

#[cfg(test)]
mod tests;

//! `build`: from pins to one archive per language.
//!
//! Helix's queries are checked out once. Each distinct grammar repository
//! is a unit of work, run in parallel with the others: one fetch, one
//! compile per subpath, then a probe, resolved queries, a manifest and
//! license notices for each language that uses it, packed into that
//! language's archive. The checkout is removed when its unit ends, so disk
//! use stays bounded by the number of workers.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;

use crate::bundle;
use crate::compile;
use crate::git::{Git, GitError};
use crate::licenses;
use crate::manifest::Manifest;
use crate::pins::Pin;
use crate::probe;
use crate::queries::{self, QueryError};

pub(crate) const HELIX_URL: &str = "https://github.com/helix-editor/helix";

/// What to build, and where.
pub(crate) struct Request<'a> {
    pub(crate) languages: Vec<&'a Pin>,
    pub(crate) helix_url: &'a str,
    pub(crate) helix_rev: &'a str,
    /// Receives each language's archive, beside any it already holds.
    pub(crate) out: &'a Path,
    pub(crate) tag: &'a str,
    pub(crate) platform: &'a str,
}

/// The languages built, and those that failed with why, each in name
/// order.
#[derive(Debug)]
pub(crate) struct Report {
    pub(crate) built: Vec<String>,
    pub(crate) failed: Vec<(String, String)>,
}

/// Why nothing was built.
#[derive(Debug)]
pub(crate) enum BuildError {
    Helix(GitError),
    NoHighlights(Vec<String>),
    Io { path: PathBuf, source: io::Error },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Helix(e) => write!(f, "Helix queries: {e}"),
            Self::NoHighlights(languages) => {
                write!(
                    f,
                    "Helix has no highlights.scm for: {}",
                    languages.join(", ")
                )
            }
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> BuildError {
    let path = path.to_path_buf();
    move |source| BuildError::Io { path, source }
}

/// The languages one grammar repository serves, at one commit.
struct Repo<'a> {
    url: &'a str,
    rev: &'a str,
    languages: Vec<&'a Pin>,
}

/// What one unit of work did for one language.
struct Outcome {
    language: String,
    archive: Result<Vec<u8>, String>,
    warnings: Vec<String>,
}

/// What every unit of work reads.
struct Context<'a> {
    git: &'a Git,
    work: &'a Path,
    queries: PathBuf,
    request: &'a Request<'a>,
}

/// Build `request.languages` into `request.out`, writing each language's
/// archive as it is built. A language that fails, or whose archive cannot be
/// written, does not stop the others; the report lists it.
pub(crate) fn build(request: &Request, git: &Git) -> Result<Report, BuildError> {
    fs::create_dir_all(request.out).map_err(io_error(request.out))?;
    let work = tempfile::Builder::new()
        .prefix("hume-grammar-build")
        .tempdir()
        .map_err(io_error(&std::env::temp_dir()))?;
    let helix = work.path().join("helix");
    git.checkout(
        request.helix_url,
        request.helix_rev,
        &helix,
        Some("runtime/queries"),
    )
    .map_err(BuildError::Helix)?;
    let queries = helix.join("runtime").join("queries");
    let no_highlights: Vec<String> = request
        .languages
        .iter()
        .filter(|pin| !queries.join(&pin.name).join("highlights.scm").is_file())
        .map(|pin| pin.name.clone())
        .collect();
    if !no_highlights.is_empty() {
        return Err(BuildError::NoHighlights(no_highlights));
    }

    let context = Context {
        git,
        work: work.path(),
        queries,
        request,
    };
    let mut report = Report {
        built: Vec::new(),
        failed: Vec::new(),
    };
    run_in_parallel(&repos(&request.languages), &context, |outcome| {
        for warning in &outcome.warnings {
            eprintln!("warning: {}: {warning}", outcome.language);
        }
        let written = outcome.archive.and_then(|archive| {
            bundle::write_archive(request.out, &outcome.language, &archive)
                .map_err(|e| format!("{}: {e}", request.out.display()))
        });
        match written {
            Ok(()) => {
                eprintln!("built {}", outcome.language);
                report.built.push(outcome.language);
            }
            Err(reason) => {
                eprintln!("failed {}: {reason}", outcome.language);
                report.failed.push((outcome.language, reason));
            }
        }
    });
    report.built.sort();
    report.failed.sort();
    Ok(report)
}

/// `languages` grouped by the repository and commit their grammar comes
/// from, so each is fetched once.
fn repos<'a>(languages: &[&'a Pin]) -> Vec<Repo<'a>> {
    let mut repos: BTreeMap<(&str, &str), Vec<&Pin>> = BTreeMap::new();
    for pin in languages {
        repos
            .entry((pin.url.as_str(), pin.rev.as_str()))
            .or_default()
            .push(pin);
    }
    repos
        .into_iter()
        .map(|((url, rev), languages)| Repo {
            url,
            rev,
            languages,
        })
        .collect()
}

/// Run every unit of work on as many threads as the machine has cores,
/// handing each language's outcome to `done`, on this thread, as its unit
/// ends.
fn run_in_parallel(repos: &[Repo], context: &Context, mut done: impl FnMut(Outcome)) {
    let next = Mutex::new(repos.iter().enumerate());
    let workers = thread::available_parallelism().map_or(1, |n| n.get());
    let (sender, receiver) = mpsc::channel();
    thread::scope(|scope| {
        for _ in 0..workers.min(repos.len()) {
            let sender = sender.clone();
            let next = &next;
            scope.spawn(move || {
                loop {
                    let Some((id, repo)) = next
                        .lock()
                        .expect("taking the next unit cannot panic")
                        .next()
                    else {
                        break;
                    };
                    for outcome in build_repo(id, repo, context) {
                        if sender.send(outcome).is_err() {
                            return;
                        }
                    }
                }
            });
        }
        drop(sender);
        receiver.into_iter().for_each(&mut done);
    });
}

/// Fetch `repo`, build each grammar in it, and pack each language that
/// uses one.
fn build_repo(id: usize, repo: &Repo, context: &Context) -> Vec<Outcome> {
    let source = context.work.join(format!("repo-{id}"));
    let fetched = context
        .git
        .checkout(repo.url, repo.rev, &source, None)
        .map_err(|e| e.to_string());
    let mut libraries: BTreeMap<&str, Result<PathBuf, String>> = BTreeMap::new();
    let outcomes = repo
        .languages
        .iter()
        .map(|pin| {
            let library = libraries.entry(pin.subpath.as_str()).or_insert_with(|| {
                fetched.clone()?;
                let library = context.work.join(format!(
                    "lib-{id}-{}.{}",
                    pin.name,
                    std::env::consts::DLL_EXTENSION
                ));
                let grammar = if pin.subpath.is_empty() {
                    source.clone()
                } else {
                    source.join(&pin.subpath)
                };
                compile::build(&grammar, &library).map_err(|e| e.to_string())?;
                Ok(library)
            });
            let mut warnings = Vec::new();
            let archive = match library {
                Ok(library) => package(pin, &source, library, context, &mut warnings),
                Err(reason) => Err(reason.clone()),
            };
            Outcome {
                language: pin.name.clone(),
                archive,
                warnings,
            }
        })
        .collect();
    if let Err(e) = fs::remove_dir_all(&source)
        && e.kind() != io::ErrorKind::NotFound
    {
        eprintln!("warning: cannot remove {}: {e}", source.display());
    }
    outcomes
}

/// `pin`'s archive: its library, its resolved queries, its manifest, and
/// the license files and notices for what it redistributes. `checkout` is
/// the grammar's repository.
fn package(
    pin: &Pin,
    checkout: &Path,
    library: &Path,
    context: &Context,
    warnings: &mut Vec<String>,
) -> Result<Vec<u8>, String> {
    let licenses = licenses::grammar(pin, checkout).map_err(|e| e.to_string())?;
    let abi = probe::abi(library, &pin.symbol).map_err(|e| e.to_string())?;
    let request = context.request;
    let manifest = Manifest {
        name: &pin.name,
        url: &pin.url,
        rev: &pin.rev,
        subpath: &pin.subpath,
        symbol: &pin.symbol,
        abi,
        platform: request.platform,
        tag: request.tag,
        helix_rev: request.helix_rev,
    };
    let library_bytes = fs::read(library).map_err(|e| format!("{}: {e}", library.display()))?;
    let mut files = vec![
        (
            "manifest.scm".to_string(),
            manifest.to_scheme().into_bytes(),
        ),
        (
            format!("{}.{}", pin.name, std::env::consts::DLL_EXTENSION),
            library_bytes,
        ),
    ];
    files.extend(licenses);
    files.push((
        "queries/NOTICE".to_string(),
        licenses::queries_notice(&pin.name, request.helix_url, request.helix_rev).into_bytes(),
    ));
    let dir = context.queries.join(&pin.name);
    let names = queries::query_files(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for file in names {
        let resolved = queries::resolve(&pin.name, |language| {
            fs::read_to_string(context.queries.join(language).join(&file)).ok()
        })
        .map_err(|e: QueryError| format!("{file}: {e}"))?;
        for parent in resolved.missing {
            warnings.push(format!("{file} inherits {parent:?}, which has no {file}"));
        }
        files.push((format!("queries/{file}"), resolved.text.into_bytes()));
    }
    bundle::archive(&files).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;

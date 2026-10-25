//! The only code that runs git: a shallow checkout of one pinned commit.

use std::fmt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

/// Runs git for checkouts, retrying a fetch that fails, since every fetch
/// goes over the network.
pub(crate) struct Git {
    /// The pause before each retry; one retry per entry.
    retry_delays: Vec<Duration>,
}

/// A git command that failed, with what git printed.
#[derive(Debug)]
pub(crate) struct GitError {
    command: String,
    output: String,
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} failed: {}", self.command, self.output.trim())
    }
}

impl Git {
    pub(crate) fn new() -> Self {
        Self {
            retry_delays: vec![Duration::from_secs(5), Duration::from_secs(20)],
        }
    }

    /// Check out commit `rev` of `url` into the empty or missing directory
    /// `dest`, with no history. With `sparse`, only that directory (and the
    /// files at the root) is checked out, and only its file contents are
    /// downloaded.
    pub(crate) fn checkout(
        &self,
        url: &str,
        rev: &str,
        dest: &Path,
        sparse: Option<&str>,
    ) -> Result<(), GitError> {
        std::fs::create_dir_all(dest).map_err(|e| GitError {
            command: format!("creating {}", dest.display()),
            output: e.to_string(),
        })?;
        run(dest, &["init", "-q"])?;
        let mut fetch = vec!["fetch", "-q", "--depth", "1"];
        if let Some(dir) = sparse {
            run(dest, &["sparse-checkout", "set", dir])?;
            fetch.push("--filter=blob:none");
        }
        fetch.extend(["--", url, rev]);
        self.retrying(|| run(dest, &fetch))?;
        // With a blob filter, the checkout downloads the file contents.
        self.retrying(|| run(dest, &["checkout", "-q", "--detach", "FETCH_HEAD"]))
    }

    fn retrying(&self, mut attempt: impl FnMut() -> Result<(), GitError>) -> Result<(), GitError> {
        for delay in &self.retry_delays {
            if attempt().is_ok() {
                return Ok(());
            }
            thread::sleep(*delay);
        }
        attempt()
    }
}

/// Run git in `dir`. Settings a user may have that would change the
/// checkout or stop to ask for credentials are overridden: files keep their
/// committed line endings on every platform, and a repository that needs a
/// login fails instead of prompting.
fn run(dir: &Path, args: &[&str]) -> Result<(), GitError> {
    let output = Command::new("git")
        .args([
            "-c",
            "core.autocrlf=false",
            "-c",
            "core.eol=lf",
            "-c",
            "credential.helper=",
            "-c",
            "advice.detachedHead=false",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .stdin(Stdio::null())
        .output();
    let command = format!("git {}", args.join(" "));
    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(GitError {
            command,
            output: String::from_utf8_lossy(&output.stderr).into_owned(),
        }),
        Err(e) => Err(GitError {
            command,
            output: e.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests;

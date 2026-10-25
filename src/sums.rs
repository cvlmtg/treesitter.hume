//! `SHA256SUMS`, in the format `sha256sum -c` checks.

use std::fmt::Write as _;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

pub(crate) const SUMS: &str = "SHA256SUMS";

/// Write `dir/SHA256SUMS` for every file in `dir` but itself and dotfiles,
/// in name order.
pub(crate) fn write(dir: &Path) -> io::Result<()> {
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let name = entry.file_name().into_string().map_err(|name| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: file name {name:?} is not UTF-8", dir.display()),
            )
        })?;
        if name != SUMS && !name.starts_with('.') {
            names.push(name);
        }
    }
    names.sort();
    let mut sums = String::new();
    for name in names {
        let digest = file_sha256_hex(&dir.join(&name))?;
        writeln!(sums, "{digest}  {name}").expect("writing to a String cannot fail");
    }
    let path = dir.join(SUMS);
    let tmp = dir.join(format!(".{SUMS}.tmp"));
    fs::write(&tmp, sums)?;
    fs::rename(tmp, path)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn file_sha256_hex(path: &Path) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1 << 16];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            return Ok(hex(&hasher.finalize()));
        }
        hasher.update(&buffer[..read]);
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut hex, byte| {
        write!(hex, "{byte:02x}").expect("writing to a String cannot fail");
        hex
    })
}

#[cfg(test)]
mod tests;

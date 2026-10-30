//! `manifest.scm`, which says what an archive holds: a Scheme association
//! list HUME reads as data.

/// Everything HUME checks before installing a grammar, and where it came
/// from.
pub(crate) struct Manifest<'a> {
    pub(crate) name: &'a str,
    pub(crate) url: &'a str,
    pub(crate) rev: &'a str,
    pub(crate) subpath: &'a str,
    pub(crate) symbol: &'a str,
    pub(crate) abi: usize,
    /// `<os>-<arch>`.
    pub(crate) platform: &'a str,
    /// The release tag of the tool that built the archive.
    pub(crate) tag: &'a str,
    /// The Helix commit the queries come from.
    pub(crate) helix_rev: &'a str,
}

impl Manifest<'_> {
    pub(crate) fn to_scheme(&self) -> String {
        let string = |key: &str, value: &str| format!("({key} . {})", scheme_string(value));
        let fields = [
            string("name", self.name),
            string("url", self.url),
            string("rev", self.rev),
            string("subpath", self.subpath),
            string("symbol", self.symbol),
            format!("(abi . {})", self.abi),
            string("platform", self.platform),
            string("tag", self.tag),
            string("helix-rev", self.helix_rev),
        ];
        format!("({})\n", fields.join("\n "))
    }
}

fn scheme_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The string `key` holds in a manifest `to_scheme` wrote, or `None` when it
/// holds none.
pub(crate) fn read_string(manifest: &str, key: &str) -> Option<String> {
    let prefix = format!("({key} . \"");
    let start = manifest.find(&prefix)? + prefix.len();
    let mut value = String::new();
    let mut chars = manifest[start..].chars();
    loop {
        match chars.next()? {
            '"' => return Some(value),
            '\\' => value.push(chars.next()?),
            c => value.push(c),
        }
    }
}

#[cfg(test)]
mod tests;

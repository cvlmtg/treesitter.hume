/// `<os>-<arch>`, as Rust names them: the platform this tool runs on, and
/// so the one it builds grammars for.
pub(crate) fn current() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

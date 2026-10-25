# treesitter.hume — Project Instructions

## What is this?
Prebuilt tree-sitter grammars for HUME. The repo owns the grammar pins, the
Helix query pin, and `hume-grammar-build`, the tool that turns a pinned
grammar into the archive HUME installs. CI runs the same tool on each release
platform; a user on any other platform runs it locally.

## Rules
- **Comments** follow the global ⛔ Comments rules.
- **Rust idioms**: no `.unwrap()` in non-test code (`clippy::unwrap_used` is
  denied in CI, tests exempt).
- **Testing**: the full suite is, from the repo root, after `cargo fmt`:
```
cargo clippy --all-targets -- -D clippy::unwrap_used && cargo test --all-targets
```
- **Commits**: [Conventional Commits](https://www.conventionalcommits.org),
  imperative mood, no trailing period. The body says why.

## Documentation audiences
The global Documentation audiences rules apply. In this repo:

1. **End users**: `README.md` (what a release holds, building grammars
   locally, releasing).
2. **Learners**: none; HUME's `docs/learning/` covers tree-sitter concepts.
3. **Source readers**: source comments.

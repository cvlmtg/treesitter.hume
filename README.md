# treesitter.hume

Prebuilt [tree-sitter](https://tree-sitter.github.io/) grammars for
[HUME](https://github.com/cvlmtg/hume), and the tool that builds them.

Each release holds every grammar HUME knows, built for Linux (x86_64),
macOS (Apple silicon) and Windows (x86_64), with the syntax queries HUME uses
alongside. On any other platform, the same tool builds the same grammars on
your machine.

## What a release holds

| File | What it is |
|---|---|
| `grammars-<os>-<arch>.tar` | Every grammar for one platform, one `<language>.tar.gz` each |
| `grammars-<os>-<arch>.index` | Where each grammar starts in that tar, and its SHA-256 |
| `hume-grammar-build-<os>-<arch>.tar.gz` (`.zip` on Windows) | The build tool |
| `SHA256SUMS` | The SHA-256 of every file above |

Each `<language>.tar.gz` holds:

- the compiled grammar, `<language>.so`, `.dylib` or `.dll`;
- `queries/`, the language's query files from [Helix](https://helix-editor.com),
  with `; inherits:` already resolved;
- `manifest.scm`, which names the language, its grammar's source and commit,
  the C symbol to load, the tree-sitter ABI, the platform, the release tag and
  the Helix commit the queries come from.

The index's first line is `treesitter.hume <tag> <os>-<arch>`. Every other
line is `<language> <offset> <size> <sha256>`, so one grammar can be fetched
without downloading the whole tar:

```
curl -fL -r <offset>-<offset + size - 1> -o rust.tar.gz \
  https://github.com/cvlmtg/treesitter.hume/releases/download/<tag>/grammars-linux-x86_64.tar
```

The download's SHA-256 must match the index line.

## Building grammars yourself

You need git and a C/C++ compiler. The tool picks the platform's usual
compiler, or the ones the `CC` and `CXX` environment variables name. On
Windows it finds the MSVC Build Tools, or clang, gcc or zig on `PATH`.

Get the tool from a release, or build it with Rust:

```
cargo install --git https://github.com/cvlmtg/treesitter.hume --tag <tag> hume-grammar-build
```

Use the release your HUME version uses: the grammars and queries come with
the tool, so each release of the tool builds that release's grammars.

Build some languages, or all of them:

```
hume-grammar-build build rust python markdown markdown.inline
hume-grammar-build build --all --out ~/hume-grammars
```

The output folder, `dist` unless `--out` names another, is laid out like a
release: a tar, its index and `SHA256SUMS`. Building into the same folder
again adds languages to it. A language that fails to build is reported at
the end, and the others are still written.

To compile a grammar HUME does not ship, from your own checkout of it:

```
hume-grammar-build compile path/to/tree-sitter-mylang mylang.so
```

## Maintaining

- `grammars.toml` pins every grammar and the Helix commit the queries come
  from. It is generated: `scripts/sync-pins.py --helix <commit>` moves to
  another Helix commit.
- `exclusions.toml` lists languages a release leaves out, each with its
  reason and, optionally, the platforms it applies to.
- To release: set the new version in `Cargo.toml`, commit, tag the commit
  `v<version>` and push the tag. The release workflow builds every
  platform, verifies each bundle and publishes the release. Running the
  workflow by hand on a branch builds and verifies without publishing.

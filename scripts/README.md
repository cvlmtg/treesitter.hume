# scripts

## import-upstream.py

```
scripts/import-upstream.py --helix <commit> --context <commit> --into <dir>
```

Writes one snapshot of Helix and nvim-treesitter-context into `<dir>`, a
worktree of the `upstream` branch: `upstream.toml`, `language-servers.toml`,
`notices/` and one `languages/<name>/` folder for each language Helix's
`languages.toml` names and each folder of Helix's `runtime/queries`. Commits
are full 40-hex SHAs; the script checks out only what it reads.

- `language.toml` and `language-servers.toml` are slices of Helix's
  `languages.toml` as written, so an upstream diff merges line for line. Each
  slice must parse to the same tables as the whole file.
- A language gets a `[grammar]` pin when its grammar has a git source and
  Helix ships a `highlights.scm` for it. A language that borrows another's
  grammar (`jsx` uses `javascript`) gets that grammar's source and symbol,
  `tree_sitter_` plus the grammar's name with `-` turned into `_`.
- `context.scm` comes only from nvim-treesitter-context. `CONTEXT_NAMES`
  maps its language names to Helix's, in folder names and in `; inherits:`
  lines; a language Helix has no folder for is left out and reported.
- A folder keeps its `[upstream]` commits until one of its files, or a
  file of a folder it inherits from, changes, so the snapshot's diff shows
  only the languages that changed.

The script prints the languages added, removed and changed.

## upstream-changes.sh

```
scripts/upstream-changes.sh <old helix> <new helix> <old context> <new context> <report>
```

Prints the Markdown the upstream workflow's pull request carries:
`import-upstream.py`'s report, then the Helix commits between the two
commits that touched each added or changed language's queries, those that
touched `languages.toml`, and the nvim-treesitter-context commits that
touched its queries.

Tests: `python3 -m unittest discover -s scripts/tests`.

## verify-bundle.sh

```
scripts/verify-bundle.sh <dir> [--glibc <floor>] [--minos <version>]
```

Checks each bundle in `<dir>` without the tool's own code: every index
line's byte range of the tar must hash to the line's digest and unpack to the
language's library, `queries/highlights.scm`, a manifest naming that
language, platform and tag, at least one license file beside
`licenses/SOURCE`, and a `queries/NOTICE` naming the MPL-2.0 and the
manifest's Helix commit, and carrying nvim-treesitter-context's license when
the archive holds a `context.scm`. The tar must hold the members the index
lists, and no others. The release workflow runs it on each platform's bundle
before uploading it, and the languages workflow on its Linux bundle.

`--glibc` runs `check-glibc-floor.sh` on each Linux library; `--minos`
compares each macOS library's minimum OS version (from `vtool`) with
`<version>`.

## check-glibc-floor.sh

```
scripts/check-glibc-floor.sh <floor> <binary>...
```

Fails when an ELF file needs a newer glibc than `<floor>`, listing the
symbols that need it. A file with no versioned glibc symbol fails too, since
that usually means the wrong file was checked. The release workflow runs it
on the Linux tool binary and, through `verify-bundle.sh`, on every grammar.

# scripts

## sync-pins.py

Regenerates `grammars.toml` from Helix's `languages.toml`. Each language
whose grammar Helix fetches from git, and for which Helix ships a
`highlights.scm`, gets one pin: the grammar's url, commit and subpath, and
its C symbol, `tree_sitter_` plus the grammar's name with `-` turned into
`_`. The script lists Helix's queries with one GitHub API request and names
each language it skips. A language that borrows another's grammar (`jsx` uses
`javascript`) gets that grammar's source and symbol, so it still builds as an
archive of its own, with its own queries.

`helix-rev` is the Helix commit both the pins and the shipped queries come
from. To move to a newer Helix:

```
scripts/sync-pins.py --helix <full 40-hex commit>
```

Without `--helix` the script re-reads the commit `grammars.toml` already
names. Tests: `python3 -m unittest discover -s scripts/tests`.

## verify-bundle.sh

```
scripts/verify-bundle.sh <dir> [--glibc <floor>] [--minos <version>]
```

Checks each bundle in `<dir>` without the tool's own code: every index
line's byte range of the tar must hash to the line's digest and unpack to the
language's library, `queries/highlights.scm`, and a manifest naming that
language, platform and tag. The tar must hold the members the index lists,
and no others. The release workflow runs it on each platform's bundle before
uploading it.

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

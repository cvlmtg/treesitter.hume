# scripts

## sync-pins.py

Regenerates `grammars.toml` from Helix's `languages.toml`. Each language
whose grammar Helix fetches from git gets one pin: the grammar's url, commit
and subpath, and its C symbol, `tree_sitter_` plus the grammar's name with
`-` turned into `_`. A language that borrows another's grammar (`jsx` uses
`javascript`) gets that grammar's source and symbol, so it still builds as an
archive of its own, with its own queries.

`helix-rev` is the Helix commit both the pins and the shipped queries come
from. To move to a newer Helix:

```
scripts/sync-pins.py --helix <full 40-hex commit>
```

Without `--helix` the script re-reads the commit `grammars.toml` already
names. Tests: `python3 -m unittest discover -s scripts/tests`.

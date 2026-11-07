#!/usr/bin/env python3
"""Write a snapshot of Helix and nvim-treesitter-context into the upstream tree.

Usage: scripts/import-upstream.py --helix <sha> --context <sha> --into <dir>

Checks out Helix's languages.toml and runtime/queries, and
nvim-treesitter-context's queries, at the given commits, and rewrites
`<dir>`'s upstream.toml, language-servers.toml, notices/ and languages/ from
them. `<dir>` is a worktree of the `upstream` branch; the script touches
nothing else in it. Prints the languages added, removed and changed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

try:
    import tomllib
except ImportError:
    try:
        import tomli as tomllib  # type: ignore[no-redef]
    except ImportError:
        sys.exit("error: requires Python 3.11+ or 'pip install tomli'")

HELIX_URL = "https://github.com/helix-editor/helix"
CONTEXT_URL = "https://github.com/nvim-treesitter/nvim-treesitter-context"

# A language name names its folder and its archive: lowercase so it stays
# distinct on case-insensitive file systems, no leading dot so it is never a
# dotfile.
LANGUAGE_NAME = re.compile(r"[a-z0-9_-][a-z0-9._-]*")

FULL_REV = re.compile(r"[0-9a-f]{40}")

# nvim-treesitter-context names that differ from Helix's for the same
# language. Any other name is taken as Helix's own.
CONTEXT_NAMES = {
    "c_sharp": "c-sharp",
    "janet_simple": "janet",
    "ocaml_interface": "ocaml-interface",
    "php_only": "php-only",
    "proto": "protobuf",
}

# The directive as hume-grammar-build's query resolver reads it.
INHERITS = re.compile(r"(;+\s*inherits\s*:?\s*)([a-z_,()-]+)")

TOP_LEVEL_KEYS = {"use-grammars", "language-server", "language", "grammar"}

HEADER = re.compile(r"\[\[?([^\]]+)\]\]?")

CONTEXT_FILE = "context.scm"


def git_grammars(doc: dict) -> dict[str, dict]:
    """`{name: source}` for each grammar with a git source."""
    grammars = {}
    for entry in doc.get("grammar", []):
        try:
            name = entry["name"]
            source = entry.get("source", {})
            if source.get("git"):
                grammars[name] = {
                    "url": source["git"],
                    "rev": source["rev"],
                    "subpath": source.get("subpath", ""),
                }
        except KeyError as e:
            sys.exit(f"error: malformed grammar entry in languages.toml: missing key {e}")
    return grammars


def parse_pins(doc: dict, highlighted: set[str]) -> dict[str, dict]:
    """`{language: pin}` for each language whose grammar has a git source and
    that is in `highlighted`.

    A language uses the grammar of its own name unless it names another, and
    the C symbol comes from the grammar's name, so a language that borrows a
    grammar (`jsx` uses `javascript`) gets that grammar's symbol."""
    grammars = git_grammars(doc)
    pins = {}
    for entry in doc.get("language", []):
        try:
            name = entry["name"]
        except KeyError as e:
            sys.exit(f"error: malformed language entry in languages.toml: missing key {e}")
        grammar_name = entry.get("grammar", name)
        grammar = grammars.get(grammar_name)
        if grammar is None:
            print(f"  no pin for '{name}': grammar '{grammar_name}' has no git source", file=sys.stderr)
            continue
        if name not in highlighted:
            print(f"  no pin for '{name}': Helix ships no highlights.scm for it", file=sys.stderr)
            continue
        if not LANGUAGE_NAME.fullmatch(name):
            sys.exit(f"error: language name '{name}' does not match {LANGUAGE_NAME.pattern}")
        if not FULL_REV.fullmatch(grammar["rev"]):
            sys.exit(f"error: grammar '{grammar_name}' rev '{grammar['rev']}' is not a full commit SHA")
        pins[name] = {**grammar, "symbol": "tree_sitter_" + grammar_name.replace("-", "_")}
    return pins


def block_text(lines: list[str]) -> str:
    return "".join(lines).rstrip("\n") + "\n"


def split_languages_toml(text: str, doc: dict) -> tuple[str, dict[str, str]]:
    """The `[language-server]` region and each `[[language]]` block of
    languages.toml, as written. Each piece must parse to the same tables as
    `doc`, the whole file parsed."""
    unknown = set(doc) - TOP_LEVEL_KEYS
    if unknown:
        sys.exit(f"error: languages.toml has unknown top-level keys: {sorted(unknown)}")

    servers: list[str] = []
    languages: list[list[str]] = []
    current: list[str] | None = None
    for line in text.splitlines(keepends=True):
        header = HEADER.match(line)
        if header:
            table = header[1].strip()
            if table == "language-server" or table.startswith("language-server."):
                if languages:
                    sys.exit(f"error: languages.toml: [{table}] follows the first [[language]]")
                current = servers
            elif table == "language" and line.startswith("[["):
                languages.append([])
                current = languages[-1]
            elif table.startswith("language."):
                if not languages:
                    sys.exit(f"error: languages.toml: [{table}] comes before any [[language]]")
                current = languages[-1]
            elif table == "grammar" and line.startswith("[["):
                current = None
            else:
                sys.exit(f"error: languages.toml: unknown table [{table}]")
        if current is not None:
            current.append(line)

    servers_text = block_text(servers)
    if tomllib.loads(servers_text).get("language-server") != doc.get("language-server"):
        sys.exit("error: languages.toml: the [language-server] region parses differently on its own")

    entries = doc.get("language", [])
    if len(languages) != len(entries):
        sys.exit(f"error: languages.toml: found {len(languages)} [[language]] blocks, parsed {len(entries)}")
    blocks = {}
    for lines, entry in zip(languages, entries):
        text = block_text(lines)
        if tomllib.loads(text) != {"language": [entry]}:
            sys.exit(f"error: languages.toml: the block for '{entry.get('name')}' parses differently on its own")
        blocks[entry["name"]] = text
    return servers_text, blocks


def helix_name(context_name: str) -> str:
    return CONTEXT_NAMES.get(context_name, context_name)


def rewrite_inherits(text: str) -> str:
    """`text` with each `; inherits:` name mapped to Helix's name."""
    return INHERITS.sub(
        lambda m: m[1] + ",".join(helix_name(name) for name in m[2].split(",")),
        text,
    )


def parents(text: str) -> set[str]:
    return {name for m in INHERITS.finditer(text) for name in m[2].split(",") if name}


@dataclass
class Folder:
    """One folder of languages/: its query files and language.toml by file
    name, and its grammar pin."""

    files: dict[str, str] = field(default_factory=dict)
    grammar: dict | None = None


def query_files(directory: Path) -> dict[str, str]:
    return {
        path.name: path.read_text(encoding="utf-8")
        for path in sorted(directory.iterdir())
        if path.is_file() and path.name.endswith(".scm")
    }


def upstream_tree(helix: Path, context: Path) -> tuple[dict[str, Folder], str, list[str]]:
    """The folders a snapshot of the two checkouts holds, the
    language-server region, and the context languages Helix has no folder
    for."""
    text = (helix / "languages.toml").read_text(encoding="utf-8")
    doc = tomllib.loads(text)
    servers_text, blocks = split_languages_toml(text, doc)

    queries = helix / "runtime" / "queries"
    helix_dirs = sorted(path for path in queries.iterdir() if path.is_dir())
    highlighted = {path.name for path in helix_dirs if (path / "highlights.scm").is_file()}
    pins = parse_pins(doc, highlighted)

    tree: dict[str, Folder] = {}
    for name, block in blocks.items():
        tree.setdefault(name, Folder()).files["language.toml"] = block
    for directory in helix_dirs:
        files = {name: text for name, text in query_files(directory).items() if name != CONTEXT_FILE}
        if files:
            tree.setdefault(directory.name, Folder()).files.update(files)
    for name, pin in pins.items():
        tree[name].grammar = pin

    skipped = []
    for directory in sorted(path for path in (context / "queries").iterdir() if path.is_dir()):
        source = directory / CONTEXT_FILE
        if not source.is_file():
            continue
        name = helix_name(directory.name)
        if name not in tree:
            skipped.append(directory.name)
            continue
        tree[name].files[CONTEXT_FILE] = rewrite_inherits(source.read_text(encoding="utf-8"))

    for name in tree:
        if not LANGUAGE_NAME.fullmatch(name):
            sys.exit(f"error: folder name '{name}' does not match {LANGUAGE_NAME.pattern}")
    return tree, servers_text, skipped


def fingerprints(tree: dict[str, Folder]) -> dict[str, tuple[str, str | None]]:
    """Each folder's Helix and context fingerprints: a hash of its files of
    that origin and of the same fingerprints of every folder it inherits
    from. The context fingerprint is None without a context.scm."""
    memo: dict[tuple[str, bool], str | None] = {}

    def fingerprint(name: str, is_context: bool, chain: tuple[str, ...]) -> str | None:
        key = (name, is_context)
        if key in memo:
            return memo[key]
        if name in chain:
            sys.exit(f"error: inheritance cycle: {' -> '.join(chain + (name,))}")
        folder = tree.get(name)
        if folder is None:
            return None
        own = {f: t for f, t in folder.files.items() if (f == CONTEXT_FILE) == is_context}
        if is_context and not own:
            memo[key] = None
            return None
        digest = hashlib.sha256()
        digest.update(json.dumps(own, sort_keys=True).encode())
        if not is_context:
            digest.update(json.dumps(folder.grammar, sort_keys=True).encode())
        for parent in sorted(set().union(*(parents(t) for t in own.values()))):
            digest.update(f"\0{parent}\0{fingerprint(parent, is_context, chain + (name,))}".encode())
        memo[key] = digest.hexdigest()
        return memo[key]

    return {name: (fingerprint(name, False, ()), fingerprint(name, True, ())) for name in tree}


def read_snapshot(languages: Path) -> tuple[dict[str, Folder], dict[str, dict]]:
    """The folders under `languages`, and each folder's `[upstream]` table."""
    tree: dict[str, Folder] = {}
    revisions: dict[str, dict] = {}
    if not languages.is_dir():
        return tree, revisions
    for directory in sorted(path for path in languages.iterdir() if path.is_dir()):
        source = tomllib.loads((directory / "source.toml").read_text(encoding="utf-8"))
        files = query_files(directory)
        language = directory / "language.toml"
        if language.is_file():
            files["language.toml"] = language.read_text(encoding="utf-8")
        tree[directory.name] = Folder(files, source.get("grammar"))
        revisions[directory.name] = source["upstream"]
    return tree, revisions


def toml_str(value: str) -> str:
    # A JSON string is a valid TOML basic string.
    return json.dumps(value, ensure_ascii=False)


def source_toml(upstream: dict, grammar: dict | None) -> str:
    lines = ["[upstream]\n"]
    lines += [f"{key} = {toml_str(upstream[key])}\n" for key in ("helix", "context") if key in upstream]
    if grammar is not None:
        lines.append("\n[grammar]\n")
        lines += [f"{key} = {toml_str(grammar[key])}\n" for key in ("url", "rev", "subpath", "symbol")]
    return "".join(lines)


@dataclass
class Report:
    added: list[str]
    removed: list[str]
    changed: list[str]
    skipped_context: list[str]


def snapshot(helix: Path, helix_rev: str, context: Path, context_rev: str, out: Path) -> Report:
    """Rewrite `out`'s snapshot from the two checkouts. A folder keeps the
    revision of each origin whose fingerprint did not change."""
    languages = out / "languages"
    old_tree, old_revisions = read_snapshot(languages)
    old_prints = fingerprints(old_tree)
    tree, servers_text, skipped = upstream_tree(helix, context)
    new_prints = fingerprints(tree)

    revisions = {}
    for name, (helix_print, context_print) in new_prints.items():
        old_helix, old_context = old_prints.get(name, (None, None))
        old = old_revisions.get(name, {})
        upstream = {"helix": old["helix"] if helix_print == old_helix else helix_rev}
        if context_print is not None:
            upstream["context"] = old["context"] if context_print == old_context else context_rev
        revisions[name] = upstream

    shutil.rmtree(languages, ignore_errors=True)
    for name, folder in tree.items():
        directory = languages / name
        directory.mkdir(parents=True)
        for file, text in folder.files.items():
            (directory / file).write_text(text, encoding="utf-8")
        (directory / "source.toml").write_text(source_toml(revisions[name], folder.grammar), encoding="utf-8")

    (out / "language-servers.toml").write_text(servers_text, encoding="utf-8")
    notices = out / "notices"
    shutil.rmtree(notices, ignore_errors=True)
    notices.mkdir()
    shutil.copyfile(context / "LICENSE", notices / "nvim-treesitter-context.LICENSE")
    (out / "upstream.toml").write_text(
        f"helix = {toml_str(helix_rev)}\ncontext = {toml_str(context_rev)}\n", encoding="utf-8"
    )

    return Report(
        added=sorted(set(tree) - set(old_tree)),
        removed=sorted(set(old_tree) - set(tree)),
        changed=sorted(name for name in set(tree) & set(old_tree) if new_prints[name] != old_prints[name]),
        skipped_context=skipped,
    )


def checkout(url: str, rev: str, dest: Path, sparse: str) -> None:
    """`url` at `rev` into `dest`: the root's files and the `sparse` folder."""
    commands = [
        ["init", "-q"],
        ["sparse-checkout", "set", sparse],
        ["fetch", "-q", "--depth", "1", "--filter=blob:none", "--", url, rev],
        ["checkout", "-q", "--detach", "FETCH_HEAD"],
    ]
    dest.mkdir()
    for args in commands:
        result = subprocess.run(["git", "-C", str(dest), *args], capture_output=True, text=True)
        if result.returncode != 0:
            sys.exit(f"error: git {' '.join(args)}: {result.stderr.strip()}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--helix", required=True, help="the Helix commit, a full 40-hex SHA")
    parser.add_argument("--context", required=True, help="the nvim-treesitter-context commit, a full 40-hex SHA")
    parser.add_argument("--into", required=True, type=Path, help="a worktree of the upstream branch")
    args = parser.parse_args()
    for flag, rev in (("--helix", args.helix), ("--context", args.context)):
        if not FULL_REV.fullmatch(rev):
            sys.exit(f"error: {flag} '{rev}' is not a full commit SHA")

    with tempfile.TemporaryDirectory(prefix="import-upstream") as work:
        helix = Path(work) / "helix"
        context = Path(work) / "context"
        checkout(HELIX_URL, args.helix, helix, "runtime/queries")
        checkout(CONTEXT_URL, args.context, context, "queries")
        report = snapshot(helix, args.helix, context, args.context, args.into)

    for label, names in (
        ("added", report.added),
        ("removed", report.removed),
        ("changed", report.changed),
        ("context languages Helix has no folder for", report.skipped_context),
    ):
        print(f"{label} ({len(names)}): {' '.join(names)}")


if __name__ == "__main__":
    main()

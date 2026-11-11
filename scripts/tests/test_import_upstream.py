"""Tests for scripts/import-upstream.py."""

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

try:
    import tomllib
except ImportError:
    import tomli as tomllib  # type: ignore[no-redef]

SCRIPTS = Path(__file__).resolve().parent.parent
_spec = importlib.util.spec_from_file_location("import_upstream", SCRIPTS / "import-upstream.py")
imp = importlib.util.module_from_spec(_spec)
# dataclasses look their module up in sys.modules.
sys.modules[_spec.name] = imp
_spec.loader.exec_module(imp)

REV_JS = "1" * 40
REV_MD = "2" * 40
HELIX_OLD = "a" * 40
HELIX_NEW = "b" * 40
CONTEXT_OLD = "c" * 40
CONTEXT_NEW = "d" * 40
FOLDS_OLD = "e" * 40
FOLDS_NEW = "f" * 40


def doc(languages, grammars):
    return {"language": languages, "grammar": grammars}


GRAMMARS = [
    {"name": "javascript", "source": {"git": "https://js", "rev": REV_JS}},
    {
        "name": "markdown_inline",
        "source": {"git": "https://md", "rev": REV_MD, "subpath": "tree-sitter-markdown-inline"},
    },
    {"name": "glimmer-javascript", "source": {"git": "https://gjs", "rev": REV_JS}},
    {"name": "local", "source": {"path": "/somewhere"}},
]


def pin_of(language):
    pins = imp.parse_pins(doc([language], GRAMMARS), {language["name"]})
    return pins.get(language["name"])


class ParsePins(unittest.TestCase):
    def test_a_language_uses_its_own_grammar(self):
        self.assertEqual(
            pin_of({"name": "javascript"}),
            {"url": "https://js", "rev": REV_JS, "subpath": "", "symbol": "tree_sitter_javascript"},
        )

    def test_a_language_borrowing_a_grammar_takes_its_source_and_symbol(self):
        self.assertEqual(
            pin_of({"name": "jsx", "grammar": "javascript"}),
            {"url": "https://js", "rev": REV_JS, "subpath": "", "symbol": "tree_sitter_javascript"},
        )

    def test_the_subpath_is_kept(self):
        pin = pin_of({"name": "markdown.inline", "grammar": "markdown_inline"})
        self.assertEqual(pin["subpath"], "tree-sitter-markdown-inline")

    def test_dashes_in_the_grammar_name_become_underscores_in_the_symbol(self):
        pin = pin_of({"name": "gjs", "grammar": "glimmer-javascript"})
        self.assertEqual(pin["symbol"], "tree_sitter_glimmer_javascript")

    def test_a_grammar_without_a_git_source_gives_no_pin(self):
        self.assertIsNone(pin_of({"name": "local"}))

    def test_a_language_without_highlights_gives_no_pin(self):
        pins = imp.parse_pins(doc([{"name": "javascript"}], GRAMMARS), set())
        self.assertEqual(pins, {})

    def test_an_abbreviated_rev_is_fatal(self):
        grammars = [{"name": "javascript", "source": {"git": "https://js", "rev": "abc123"}}]
        with self.assertRaises(SystemExit):
            imp.parse_pins(doc([{"name": "javascript"}], grammars), {"javascript"})

    def test_a_name_that_is_not_a_language_name_is_fatal(self):
        with self.assertRaises(SystemExit):
            imp.parse_pins(doc([{"name": "Java Script", "grammar": "javascript"}], GRAMMARS), {"Java Script"})


LANGUAGES_TOML = """\
# Language support configuration.

use-grammars = { except = [ "wren" ] }

[language-server]
rust-analyzer = { command = "rust-analyzer" }

[language-server.pylsp]
command = "pylsp"

[[language]]
name = "rust"
file-types = ["rs"]
language-servers = ["rust-analyzer"]

[language.auto-pairs]
'(' = ')'

[[grammar]]
name = "rust"
source = { git = "https://rust", rev = "3333333333333333333333333333333333333333" }

[[language]]
name = "python"
file-types = ["py"]
# trailing comment
[[language.debugger.templates]]
name = "binary"
"""


class SplitLanguagesToml(unittest.TestCase):
    def split(self, text):
        return imp.split_languages_toml(text, tomllib.loads(text))

    def test_the_server_region_is_kept_as_written(self):
        servers, _ = self.split(LANGUAGES_TOML)
        self.assertEqual(
            servers,
            '[language-server]\nrust-analyzer = { command = "rust-analyzer" }\n\n'
            '[language-server.pylsp]\ncommand = "pylsp"\n',
        )

    def test_each_language_block_keeps_its_sub_tables_and_stops_at_a_grammar(self):
        _, blocks = self.split(LANGUAGES_TOML)
        self.assertEqual(
            blocks["rust"],
            '[[language]]\nname = "rust"\nfile-types = ["rs"]\nlanguage-servers = ["rust-analyzer"]\n\n'
            "[language.auto-pairs]\n'(' = ')'\n",
        )
        self.assertEqual(
            tomllib.loads(blocks["python"]),
            {"language": [{"name": "python", "file-types": ["py"], "debugger": {"templates": [{"name": "binary"}]}}]},
        )

    def test_a_block_that_parses_differently_from_the_whole_file_is_fatal(self):
        parsed = tomllib.loads(LANGUAGES_TOML)
        parsed["language"][0]["file-types"] = ["other"]
        with self.assertRaises(SystemExit):
            imp.split_languages_toml(LANGUAGES_TOML, parsed)

    def test_an_unknown_table_header_is_fatal(self):
        with self.assertRaises(SystemExit):
            self.split(LANGUAGES_TOML + '\n[editor]\nmouse = false\n')

    def test_an_unknown_top_level_key_is_fatal(self):
        with self.assertRaises(SystemExit):
            self.split('theme = "x"\n' + LANGUAGES_TOML)


class NvimNames(unittest.TestCase):
    def test_nvim_names_map_to_helix_names(self):
        self.assertEqual(imp.helix_name("c_sharp"), "c-sharp")
        self.assertEqual(imp.helix_name("query"), "tsq")
        self.assertEqual(imp.helix_name("rust"), "rust")

    def test_inherits_names_are_mapped(self):
        self.assertEqual(
            imp.rewrite_inherits("; inherits: c_sharp,php_only\n(x) @context\n"),
            "; inherits: c-sharp,php-only\n(x) @context\n",
        )

    def test_text_without_a_directive_is_unchanged(self):
        text = "; a comment naming c_sharp\n(x) @context\n"
        self.assertEqual(imp.rewrite_inherits(text), text)


def write(root, files):
    for path, text in files.items():
        target = root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")


class Snapshot(unittest.TestCase):
    """Builds a snapshot from fake Helix, nvim-treesitter-context and
    nvim-treesitter trees."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.helix = root / "helix"
        self.context = root / "context"
        self.folds = root / "folds"
        self.out = root / "out"
        self.out.mkdir()
        write(
            self.helix,
            {
                "languages.toml": LANGUAGES_TOML,
                "runtime/queries/rust/highlights.scm": "(identifier) @variable\n",
                "runtime/queries/rust/README.md": "not a query\n",
                "runtime/queries/python/highlights.scm": "; inherits: base\n(identifier) @variable\n",
                "runtime/queries/python/context.scm": "(helix) @context\n",
                "runtime/queries/python/folds.scm": "; inherits: base\n(helix) @fold\n",
                "runtime/queries/rust/folds.scm": "(helix) @fold\n",
                "runtime/queries/base/highlights.scm": "(comment) @comment\n",
            },
        )
        write(
            self.context,
            {
                "LICENSE": "MIT text\n",
                "queries/rust/context.scm": "(function_item) @context\n",
                "queries/python/context.scm": "(function_definition) @context\n",
                "queries/terraform/context.scm": "(block) @context\n",
            },
        )
        write(
            self.folds,
            {
                "LICENSE": "Apache text\n",
                "runtime/queries/rust/folds.scm": "(function_item) @fold\n",
                "runtime/queries/base/folds.scm": "; inherits: php_only\n(comment) @fold\n",
                "runtime/queries/rust/highlights.scm": "(nvim) @variable\n",
                "runtime/queries/terraform/folds.scm": "(block) @fold\n",
            },
        )

    def tearDown(self):
        self.tmp.cleanup()

    def snapshot(self, helix_rev, context_rev, folds_rev):
        return imp.snapshot(
            self.helix, helix_rev, self.context, context_rev, self.folds, folds_rev, self.out
        )

    def source(self, name):
        return tomllib.loads((self.out / "languages" / name / "source.toml").read_text())

    def test_the_first_snapshot_writes_the_layout(self):
        self.snapshot(HELIX_OLD, CONTEXT_OLD, FOLDS_OLD)
        self.assertEqual(
            self.source("rust"),
            {
                "upstream": {"helix": HELIX_OLD, "context": CONTEXT_OLD, "folds": FOLDS_OLD},
                "grammar": {
                    "url": "https://rust",
                    "rev": "3" * 40,
                    "subpath": "",
                    "symbol": "tree_sitter_rust",
                },
            },
        )
        self.assertEqual(self.source("base"), {"upstream": {"helix": HELIX_OLD, "folds": FOLDS_OLD}})
        rust = self.out / "languages" / "rust"
        self.assertEqual(
            sorted(p.name for p in rust.iterdir()),
            ["context.scm", "folds.scm", "highlights.scm", "language.toml", "source.toml"],
        )
        self.assertEqual(
            tomllib.loads((self.out / "upstream.toml").read_text()),
            {"helix": HELIX_OLD, "context": CONTEXT_OLD, "folds": FOLDS_OLD},
        )
        self.assertEqual((self.out / "notices" / "nvim-treesitter-context.LICENSE").read_text(), "MIT text\n")
        self.assertEqual((self.out / "notices" / "nvim-treesitter.LICENSE").read_text(), "Apache text\n")
        self.assertTrue((self.out / "language-servers.toml").read_text().startswith("[language-server]\n"))

    def test_context_queries_come_from_nvim_treesitter_context_only(self):
        self.snapshot(HELIX_OLD, CONTEXT_OLD, FOLDS_OLD)
        self.assertEqual(
            (self.out / "languages" / "python" / "context.scm").read_text(),
            "(function_definition) @context\n",
        )

    def test_a_context_language_helix_lacks_is_left_out(self):
        report = self.snapshot(HELIX_OLD, CONTEXT_OLD, FOLDS_OLD)
        self.assertFalse((self.out / "languages" / "terraform").exists())
        self.assertEqual(report.skipped_context, ["terraform"])

    def test_folds_queries_come_from_nvim_treesitter_when_it_has_them(self):
        report = self.snapshot(HELIX_OLD, CONTEXT_OLD, FOLDS_OLD)
        languages = self.out / "languages"
        self.assertEqual((languages / "rust" / "folds.scm").read_text(), "(function_item) @fold\n")
        self.assertEqual((languages / "rust" / "highlights.scm").read_text(), "(identifier) @variable\n")
        self.assertEqual((languages / "python" / "folds.scm").read_text(), "; inherits: base\n(helix) @fold\n")
        self.assertNotIn("folds", self.source("python")["upstream"])
        self.assertEqual((languages / "base" / "folds.scm").read_text(), "; inherits: php-only\n(comment) @fold\n")
        self.assertEqual(report.skipped_folds, ["terraform"])

    def test_an_unchanged_language_keeps_its_revisions(self):
        self.snapshot(HELIX_OLD, CONTEXT_OLD, FOLDS_OLD)
        report = self.snapshot(HELIX_NEW, CONTEXT_NEW, FOLDS_NEW)
        self.assertEqual(
            self.source("rust")["upstream"],
            {"helix": HELIX_OLD, "context": CONTEXT_OLD, "folds": FOLDS_OLD},
        )
        self.assertEqual(report.changed, [])
        self.assertEqual(
            tomllib.loads((self.out / "upstream.toml").read_text()),
            {"helix": HELIX_NEW, "context": CONTEXT_NEW, "folds": FOLDS_NEW},
        )

    def test_a_folds_change_moves_the_folds_revision_only(self):
        self.snapshot(HELIX_OLD, CONTEXT_OLD, FOLDS_OLD)
        write(self.folds, {"runtime/queries/rust/folds.scm": "(impl_item) @fold\n"})
        self.snapshot(HELIX_NEW, CONTEXT_NEW, FOLDS_NEW)
        self.assertEqual(
            self.source("rust")["upstream"],
            {"helix": HELIX_OLD, "context": CONTEXT_OLD, "folds": FOLDS_NEW},
        )

    def test_a_helix_folds_query_nvim_replaces_moves_no_revision(self):
        self.snapshot(HELIX_OLD, CONTEXT_OLD, FOLDS_OLD)
        write(self.helix, {"runtime/queries/rust/folds.scm": "(changed) @fold\n"})
        report = self.snapshot(HELIX_NEW, CONTEXT_NEW, FOLDS_NEW)
        self.assertEqual(self.source("rust")["upstream"]["helix"], HELIX_OLD)
        self.assertEqual(report.changed, [])

    def test_a_helix_folds_query_inheriting_an_nvim_one_follows_its_change(self):
        self.snapshot(HELIX_OLD, CONTEXT_OLD, FOLDS_OLD)
        write(self.folds, {"runtime/queries/base/folds.scm": "(block_comment) @fold\n"})
        report = self.snapshot(HELIX_NEW, CONTEXT_NEW, FOLDS_NEW)
        self.assertEqual(self.source("python")["upstream"]["helix"], HELIX_NEW)
        self.assertEqual(self.source("base")["upstream"]["folds"], FOLDS_NEW)
        self.assertEqual(report.changed, ["base", "python"])

    def test_a_parent_change_moves_the_child_helix_revision_only(self):
        self.snapshot(HELIX_OLD, CONTEXT_OLD, FOLDS_OLD)
        write(self.helix, {"runtime/queries/base/highlights.scm": "(comment) @comment.line\n"})
        report = self.snapshot(HELIX_NEW, CONTEXT_NEW, FOLDS_NEW)
        self.assertEqual(self.source("python")["upstream"], {"helix": HELIX_NEW, "context": CONTEXT_OLD})
        self.assertEqual(self.source("rust")["upstream"]["helix"], HELIX_OLD)
        self.assertEqual(report.changed, ["base", "python"])

    def test_a_context_change_moves_the_context_revision_only(self):
        self.snapshot(HELIX_OLD, CONTEXT_OLD, FOLDS_OLD)
        write(self.context, {"queries/rust/context.scm": "(impl_item) @context\n"})
        self.snapshot(HELIX_NEW, CONTEXT_NEW, FOLDS_NEW)
        self.assertEqual(
            self.source("rust")["upstream"],
            {"helix": HELIX_OLD, "context": CONTEXT_NEW, "folds": FOLDS_OLD},
        )

    def test_a_language_upstream_dropped_is_removed(self):
        self.snapshot(HELIX_OLD, CONTEXT_OLD, FOLDS_OLD)
        (self.helix / "runtime/queries/base/highlights.scm").unlink()
        (self.helix / "runtime/queries/base").rmdir()
        report = self.snapshot(HELIX_NEW, CONTEXT_NEW, FOLDS_NEW)
        self.assertFalse((self.out / "languages" / "base").exists())
        self.assertEqual(report.removed, ["base"])


if __name__ == "__main__":
    unittest.main()

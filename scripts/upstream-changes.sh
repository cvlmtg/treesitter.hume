#!/usr/bin/env bash
# Markdown describing a new upstream snapshot: import-upstream.py's report,
# then the Helix commits that touched each added or changed language's
# queries, those that touched languages.toml, the nvim-treesitter-context
# commits that touched its queries, and the nvim-treesitter commits that
# touched a folds.scm.
# Usage: upstream-changes.sh <old helix> <new helix> <old context> <new context> <old folds> <new folds> <report>
set -euo pipefail

old_helix="$1"
new_helix="$2"
old_context="$3"
new_context="$4"
old_folds="$5"
new_folds="$6"
report="$7"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git clone -q --filter=blob:none --no-checkout https://github.com/helix-editor/helix "$work/helix"
git clone -q --filter=blob:none --no-checkout https://github.com/nvim-treesitter/nvim-treesitter-context "$work/context"
git clone -q --filter=blob:none --no-checkout https://github.com/nvim-treesitter/nvim-treesitter "$work/folds"

# commits <repo> <from> <to> <path>...
commits() {
    git -C "$1" log --no-merges --format='- %s (%h)' "$2..$3" -- "${@:4}"
}

# section <title> <commit list>
section() {
    [[ -n "$2" ]] && printf '\n### %s\n\n%s\n' "$1" "$2"
    return 0
}

echo "Helix $old_helix → $new_helix"
echo "nvim-treesitter-context $old_context → $new_context"
echo "nvim-treesitter $old_folds → $new_folds"
printf '\n```\n%s\n```\n' "$(cat "$report")"
for name in $(awk -F': ' '/^(added|changed) \(/ { print $2 }' "$report"); do
    section "$name" "$(commits "$work/helix" "$old_helix" "$new_helix" "runtime/queries/$name")"
done
section "languages.toml" "$(commits "$work/helix" "$old_helix" "$new_helix" languages.toml)"
section "nvim-treesitter-context" "$(commits "$work/context" "$old_context" "$new_context" queries)"
section "nvim-treesitter" "$(commits "$work/folds" "$old_folds" "$new_folds" ':(glob)runtime/queries/*/folds.scm')"

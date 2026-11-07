#!/usr/bin/env bash
# Markdown describing a new upstream snapshot: import-upstream.py's report,
# then the Helix commits that touched each added or changed language's
# queries, those that touched languages.toml, and the nvim-treesitter-context
# commits that touched its queries.
# Usage: upstream-changes.sh <old helix> <new helix> <old context> <new context> <report>
set -euo pipefail

old_helix="$1"
new_helix="$2"
old_context="$3"
new_context="$4"
report="$5"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
git clone -q --filter=blob:none --no-checkout https://github.com/helix-editor/helix "$work/helix"
git clone -q --filter=blob:none --no-checkout https://github.com/nvim-treesitter/nvim-treesitter-context "$work/context"

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
printf '\n```\n%s\n```\n' "$(cat "$report")"
for name in $(awk -F': ' '/^(added|changed) \(/ { print $2 }' "$report"); do
    section "$name" "$(commits "$work/helix" "$old_helix" "$new_helix" "runtime/queries/$name")"
done
section "languages.toml" "$(commits "$work/helix" "$old_helix" "$new_helix" languages.toml)"
section "nvim-treesitter-context" "$(commits "$work/context" "$old_context" "$new_context" queries)"

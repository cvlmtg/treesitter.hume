#!/usr/bin/env bash
# Check every bundle in <dir> through its index alone, as a client reads it:
# each member's byte range must hash to its digest and unpack to the
# language's library, queries and a manifest naming that language, platform
# and tag, to its grammar's license files beside licenses/SOURCE, and to a
# queries/NOTICE naming the MPL-2.0 and the manifest's Helix commit.
# --glibc checks each Linux library against a glibc floor, --minos each
# macOS library's minimum OS version.
# Usage: verify-bundle.sh <dir> [--glibc <floor>] [--minos <version>]
set -euo pipefail

scripts="$(cd "$(dirname "$0")" && pwd)"
dir="$1"
shift
glibc=""
minos=""
while (($#)); do
    case "$1" in
        --glibc) glibc="$2"; shift 2 ;;
        --minos) minos="$2"; shift 2 ;;
        *) echo "error: unknown option $1" >&2; exit 2 ;;
    esac
done

sha256() {
    if command -v sha256sum >/dev/null; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

fail() {
    echo "error: $*" >&2
    exit 1
}

shopt -s nullglob
indexes=("$dir"/grammars-*.index)
((${#indexes[@]})) || fail "no grammars-*.index in $dir"

for index in "${indexes[@]}"; do
    read -r magic tag platform < "$index"
    [[ "$magic" == treesitter.hume && -n "$tag" && -n "$platform" ]] \
        || fail "$index: bad header"
    tar_file="$dir/grammars-$platform.tar"
    [[ -f "$tar_file" ]] || fail "$index names $tar_file, which is missing"
    case "$platform" in
        macos-*) ext=dylib ;;
        windows-*) ext=dll ;;
        *) ext=so ;;
    esac

    listed="$(tail -n +2 "$index" | cut -d' ' -f1 | sed 's/$/.tar.gz/')"
    [[ "$(tar tf "$tar_file")" == "$listed" ]] \
        || fail "$tar_file members differ from $index"

    count=0
    while read -r language offset size digest; do
        member="$work/$language.tar.gz"
        # head stops reading at the member's end, which kills tail with
        # SIGPIPE; a short read still fails the digest check below.
        { tail -c +"$((offset + 1))" "$tar_file" || true; } | head -c "$size" > "$member"
        [[ "$(sha256 "$member")" == "$digest" ]] \
            || fail "$language: bytes $offset+$size of $tar_file do not match the index"
        unpacked="$work/$language"
        mkdir "$unpacked"
        tar xzf "$member" -C "$unpacked"
        library="$unpacked/$language.$ext"
        [[ -f "$library" ]] || fail "$language: no $language.$ext"
        [[ -f "$unpacked/queries/highlights.scm" ]] || fail "$language: no queries/highlights.scm"
        grep -qF "(name . \"$language\")" "$unpacked/manifest.scm" \
            || fail "$language: manifest names another language"
        grep -qF "(platform . \"$platform\")" "$unpacked/manifest.scm" \
            || fail "$language: manifest names another platform"
        grep -qF "(tag . \"$tag\")" "$unpacked/manifest.scm" \
            || fail "$language: manifest names another tag"
        [[ -f "$unpacked/licenses/SOURCE" ]] || fail "$language: no licenses/SOURCE"
        find "$unpacked/licenses" -type f ! -path "$unpacked/licenses/SOURCE" | grep -q . \
            || fail "$language: no license file under licenses/"
        helix_rev="$(grep -o '(helix-rev \. "[0-9a-f]*")' "$unpacked/manifest.scm" | cut -d'"' -f2)"
        [[ -n "$helix_rev" ]] || fail "$language: manifest names no Helix commit"
        [[ -f "$unpacked/queries/NOTICE" ]] || fail "$language: no queries/NOTICE"
        grep -qF "Mozilla Public License 2.0" "$unpacked/queries/NOTICE" \
            && grep -qF "$helix_rev" "$unpacked/queries/NOTICE" \
            || fail "$language: queries/NOTICE does not name the MPL-2.0 and Helix $helix_rev"
        if [[ -n "$glibc" ]]; then
            bash "$scripts/check-glibc-floor.sh" "$glibc" "$library" >/dev/null \
                || fail "$language: needs a newer glibc than $glibc"
        fi
        if [[ -n "$minos" ]]; then
            found="$(vtool -show-build "$library" | awk '$1 == "minos" { print $2 }')"
            [[ "$found" == "$minos" ]] || fail "$language: minos $found, not $minos"
        fi
        rm -rf "$member" "$unpacked"
        count=$((count + 1))
    done < <(tail -n +2 "$index")
    echo "$index: $count languages verified"
done

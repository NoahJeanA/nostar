#!/bin/sh
# Developer tool (TECHNIK.md 9.7, 14.1): patches fonts with the full dictionary, checks the
# test cases and all dictionary forms with HarfBuzz (C) and measures the extra time per MB
# against the limits of fachl. Spezifikation 8.3.
#
#   tools/bench.sh <dictionary.js> <font.ttf>...
#
# Needs HarfBuzz development files (pkg-config harfbuzz). Writes to $BENCH_OUT
# (default: target/bench). Exit code 1 if a check or a limit fails.
set -eu
[ $# -ge 2 ] || { echo "usage: tools/bench.sh <dictionary.js> <font.ttf>..." >&2; exit 2; }
root=$(cd "$(dirname "$0")/.." && pwd)
out=${BENCH_OUT:-$root/target/bench}
dict=$(realpath "$1"); shift
mkdir -p "$out"
cc -O2 -o "$out/hbcmp" "$root/tools/hbcmp.c" $(pkg-config --cflags --libs harfbuzz)
fonts=""
for f in "$@"; do fonts="$fonts${fonts:+:}$(realpath "$f")"; done
(cd "$root" && NOSTAR_TEST_DICTIONARY="$dict" NOSTAR_BENCH_FONTS="$fonts" NOSTAR_BENCH_OUT="$out" \
    cargo test --quiet --release --test font -- --ignored --exact bench_fonts --nocapture)

status=0
# limits in ms per MB: prose, code, text with gender forms
check() { # <text> <limit> <original> <patched>
    set -- $("$out/hbcmp" time "$3" "$4" "$1") "$1" "$2"
    awk -v o="$1" -v p="$2" -v b="$3" -v t="$(basename "$4")" -v l="$5" 'BEGIN {
        d = (p - o) / (b / 1e6); ok = d <= l ? "ok" : "EXCEEDED"
        printf "  %-11s +%5.1f ms/MB (limit %d)  %s\n", t, d, l, ok; exit d > l }'
}
same() { # <font original> <font patched> <in> <expected> <label>
    n=$(paste -d'|' "$($out/hbcmp shape "$2" "$3" > "$out/got"; echo "$out/got")" \
        "$($out/hbcmp shape "$1" "$4" > "$out/want"; echo "$out/want")" | awk -F'|' '$1 != $2' | wc -l)
    echo "  $5: $n of $(wc -l < "$3") differ"
    [ "$n" -eq 0 ]
}
for f in "$@"; do
    p="$out/$(basename "$f")"
    echo "$(basename "$f")"
    stem=$(basename "$f" | sed 's/\.[^.]*$//')
    same "$f" "$p" "$out/$stem.cases_in.txt" "$out/$stem.cases_exp.txt" "cases" || status=1
    same "$f" "$p" "$out/forms_in.txt" "$out/forms_exp.txt" "dictionary forms" || status=1
    check "$root/tests/bench/prose.txt" 25 "$f" "$p" || status=1
    check "$root/tests/bench/code.txt" 40 "$f" "$p" || status=1
    check "$out/marked.txt" 100 "$f" "$p" || status=1
done
exit $status

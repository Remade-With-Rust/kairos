#!/bin/sh
# Every source line in the list arm that costs more than a threshold, scaled
# to instructions PER ROUND so the numbers can be reasoned about directly
# against `run.sh`'s per-round total.
#
#     wsl -e sh bench/list-cost/lines.sh [min-per-round] [rounds]
#
# `census.sh` prints the three censuses at a readable threshold; this is the
# long tail, for when the question is "where are the remaining N instructions"
# rather than "what is hot". Run `census.sh` first — it is what produces the
# callgrind file this reads.
set -eu

MIN=${1:-2}
ROUNDS=${2:-100000}
OUT=${OUT:-/tmp/kairos-list-census/cg.rust.out}

[ -f "$OUT" ] || { echo "no $OUT — run census.sh first" >&2; exit 2; }

cd "$(dirname "$0")/../.."
callgrind_annotate --auto=yes --threshold=99.9 "$OUT" 2>/dev/null \
    | grep -E '^ *[0-9][0-9,]* ' \
    | sed 's/(\([^)]*\))//' \
    | awk -v min="$MIN" -v rounds="$ROUNDS" '
        {
            n = $1; gsub(/,/, "", n);
            per = n / rounds;
            if (per + 0 < min + 0) next;
            rest = $0;
            sub(/^ *[0-9,]+ */, "", rest);
            printf "%8.1f  %s\n", per, rest;
        }' \
    | sort -rn

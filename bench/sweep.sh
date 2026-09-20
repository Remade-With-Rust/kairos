#!/bin/sh
# Price ONE change to rusty_rtos_core's lists on every instrument that can see it.
#
#     wsl -e sh bench/sweep.sh <variant>          # from the umbrella root
#
# where <variant> names a file `bench/variants/<variant>.rs` holding the
# candidate `list.rs`. The script measures the committed HEAD and the variant
# on four instruments and prints them side by side. It changes nothing: the
# working tree is restored to HEAD when it finishes.
#
# ---- why FOUR ------------------------------------------------------------
#
# Because two of them disagreed, in sign, about a real change -- and the one
# that was easy to reach for was the one that was wrong.
#
# The tail fast path (answer a sorted insert from the last item instead of
# walking to it) measured -11.8% on list-ir and +4.78% on kdelay-ir. list-ir
# is lists-only and insert-heavy; the kernel is overwhelmingly `insert_end`
# and `remove` on ready lists, which pay whatever a change costs the other
# operations and collect nothing of what it saves. A 12% stage-level win was
# a 5% system-level loss, and only the second instrument could say so.
#
#   list-ir x86_64   the lists alone, on the host
#   list-ir i686     the lists alone, on a 32-bit machine -- every Kairos
#                    target is one, and the two arms have disagreed in SIGN
#                    (padding `Node<u32>` read +3.5% here and -3.15% there)
#   kdelay-ir        the kernel with tasks actually BLOCKED, so the sorted
#                    insert -- the only walk in list.c -- runs at all
#   ksched-ir        the kernel's ready-list traffic, which is most of what
#                    a scheduler does to a list
#
# Four is the floor, not a thorough job. `--features deep` on kdelay-ir takes
# the delayed list from depth 4 to 17, and a change to the sorted insert can
# only be priced against a stated depth.
#
# ---- and why it refuses to run on a missing variant ----------------------
#
# Because a missing variant does not look like an error, it looks like a
# RESULT. When the copy below failed once, the arm profiled whatever was
# already in the tree -- HEAD -- and printed a full set of numbers 82
# instructions from the baseline. That reads exactly like "measured, flat,
# refuted", and a wrong refutation is permanent: nothing revisits it, and the
# note reads authoritative because it has a number in it.
# ---- its own resolution, measured -----------------------------------------
#
# Run against a BYTE COPY of the committed file -- a null arm, both sides the
# same code -- the four instruments came back:
#
#   list-ir x86_64   16,781,328  vs  16,781,246     82
#   list-ir i686     19,230,909  vs  19,230,897     12
#   kdelay-ir         2,202,578  vs   2,202,582      4
#   ksched-ir         1,823,487  vs   1,823,487      0
#
# So a difference under ~100 instructions on list-ir, or ~10 on the kernel
# arms, is this harness and not your change. Quote the floor beside the
# delta, and re-take it on a new machine -- a harness that will not state its
# own resolution can report any effect you like.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
V=${1:-}
[ -n "$V" ] || { echo "usage: sh bench/sweep.sh <variant>   (bench/variants/<variant>.rs)" >&2; exit 1; }

VAR="$here/bench/variants/$V.rs"
LIST="$here/rusty_rtos_core/crates/rusty_rtos_core/src/list.rs"
[ -f "$VAR" ] || { echo "MISSING VARIANT $VAR -- refusing to profile" >&2; exit 1; }

# ...and it must not BE the committed file. A variant identical to HEAD is a
# null arm wearing a candidate's name: it produces two full columns that agree
# to within the noise floor, which reads as "measured, flat, refuted". That
# has happened twice here -- once because a `cp` silently did not run, and
# once because this script restores the tree on exit and an edit was applied
# before the restore rather than after it.
if cmp -s "$VAR" "$LIST"; then
    echo "VARIANT $V IS IDENTICAL TO HEAD -- that is a null arm, not a change" >&2
    exit 1
fi

restore() { (cd "$here/rusty_rtos_core" && git checkout -- crates/rusty_rtos_core/src/list.rs); }
trap restore EXIT

count() { # <dir> <bin> <label> [build flags...]
    d=$1; b=$2; label=$3
    shift 3
    cd "$d"
    rm -f "$b"
    cargo build --release "$@" >/dev/null 2>&1 || { echo "$label BUILD FAIL"; return 0; }
    [ -f "$b" ] || { echo "$label NO BINARY"; return 0; }
    valgrind --tool=callgrind --cache-sim=no --branch-sim=no \
             --callgrind-out-file=/tmp/sweep.out "./$b" >/tmp/sweep.txt 2>/dev/null
    printf '%-8s %-10s %-7s %12s   %s\n' "$arm" "$label" "$mach" \
        "$(grep -m1 '^summary:' /tmp/sweep.out | awk '{print $2}')" \
        "$(head -1 /tmp/sweep.txt)"
}

for arm in head "$V"; do
    if [ "$arm" = head ]; then
        restore
    else
        cp "$VAR" "$LIST" || { echo "COPY FAILED for $arm -- refusing to profile" >&2; exit 1; }
    fi

    mach=x86_64
    count "$here/rusty_rtos_core/bench/list-ir" target/release/list-ir list-ir
    mach=i686
    count "$here/rusty_rtos_core/bench/list-ir" \
          target/i686-unknown-linux-gnu/release/list-ir list-ir \
          --target i686-unknown-linux-gnu
    mach=x86_64
    count "$here/rusty_rtos_kernel/bench/kdelay-ir" target/release/kdelay-ir kdelay-ir
    count "$here/rusty_rtos_kernel/bench/ksched-ir" target/release/ksched-ir ksched-ir
done

echo
echo "Checksums must be identical down each column. A changed checksum is a"
echo "changed ANSWER, and an unchanged one is not proof of unchanged WORK --"
echo "a closed-form update produces the right total from no work at all."

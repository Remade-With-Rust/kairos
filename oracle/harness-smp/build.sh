#!/bin/sh
# Build the TWO-CORE oracle: the pinned FreeRTOS kernel with
# configNUMBER_OF_CORES 2 on the two-core sim port (this directory), running
# the standard demo tasks through the one-core harness's main.c.
#
#   sh oracle/harness-smp/build.sh                 # from the umbrella root, under WSL
#   oracle/build/smp/corpus <scenario> <ticks> 2> <scenario>.trace
#
# The 32-bit twin (HOLES.md H13: every target is 32-bit, and the width is
# observable):
#
#   KAIROS_SMP_OUT=oracle/build/smp-w32 \
#   KAIROS_SMP_CFLAGS="-m32 -isystem /usr/include/x86_64-linux-gnu" sh oracle/harness-smp/build.sh
set -eu
root=$(cd "$(dirname "$0")/../.." && pwd)
k="$root/oracle/FreeRTOS-Kernel"
d="$root/oracle/FreeRTOS/FreeRTOS/Demo/Common"
here="$root/oracle/harness-smp"
[ -f "$k/tasks.c" ] || { echo "no FreeRTOS-Kernel -- run \`kairos oracle fetch\` first" >&2; exit 1; }
python3 "$here/gen_api_hooks.py" "$k/include/FreeRTOS.h" > "$here/api_hooks.h"
python3 "$here/gen_config.py" "$root/oracle/harness/FreeRTOSConfig.h" > "$here/FreeRTOSConfig.h"
out=${KAIROS_SMP_OUT:-$root/oracle/build/smp}
case "$out" in /*) ;; *) out="$root/$out" ;; esac
mkdir -p "$out"
demos=""
for f in BlockQ dynamic PollQ semtest countsem recmutex blocktim QPeek GenQTest \
         TaskNotify AbortDelay QueueOverwrite QueueSetPolling QueueSet IntQueue \
         IntSemTest StreamBufferDemo MessageBufferDemo StreamBufferInterrupt \
         TimerDemo EventGroupsDemo death; do
  demos="$demos $d/Minimal/$f.c"
done
# shellcheck disable=SC2086
cc -O0 -g -Wall -Wno-unused-parameter -DprojCOVERAGE_TEST=0 -DKAIROS_SMP=1 ${KAIROS_SMP_CFLAGS:-} \
   -I"$here" -I"$root/oracle/harness" -I"$k/include" -I"$d/include" \
   -o "$out/corpus" -pthread \
   "$k/tasks.c" "$k/queue.c" "$k/list.c" "$k/timers.c" "$k/event_groups.c" "$k/stream_buffer.c" \
   "$k/portable/MemMang/heap_3.c" \
   "$here/port.c" "$root/oracle/harness/main.c" "$root/oracle/harness/kairos_trace.c" \
   "$root/oracle/harness/ApiSweep.c" $demos
echo "built $out/corpus"
# MessageBufferAMP replaces sbSEND_COMPLETED, a global macro, so it gets a
# binary of its own -- as the one-core oracle does (`kairos oracle build`).
# shellcheck disable=SC2086
cc -O0 -g -Wall -Wno-unused-parameter -DprojCOVERAGE_TEST=0 -DKAIROS_SMP=1 -DKAIROS_AMP=1 ${KAIROS_SMP_CFLAGS:-}    -I"$here" -I"$root/oracle/harness" -I"$k/include" -I"$d/include"    -o "$out/corpus-amp" -pthread    "$k/tasks.c" "$k/queue.c" "$k/list.c" "$k/timers.c" "$k/event_groups.c" "$k/stream_buffer.c"    "$k/portable/MemMang/heap_3.c"    "$here/port.c" "$root/oracle/harness/main.c" "$root/oracle/harness/kairos_trace.c"    "$root/oracle/harness/ApiSweep.c" $demos "$d/Minimal/MessageBufferAMP.c"
echo "built $out/corpus-amp"

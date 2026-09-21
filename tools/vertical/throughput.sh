#!/bin/sh
# iperf-style throughput rows: our stack against the Linux kernel's.
#
#     sudo sh tools/vertical/throughput.sh
#
# Both directions, because a stack can be fast writing and slow reading.
# The peer COUNTS what it saw and the count is printed beside ours, so a
# transfer that silently truncated cannot be reported as a fast one.
#
# What this is NOT: a userspace poll loop over a TAP on a workstation is
# not a NIC on a chip. These rows bound nothing about embedded
# performance. They are a baseline a later change can be measured
# against, and evidence that bulk transfer works at all.
set -eu

TAP=${TAP:-kairos2}
THEIRS=${THEIRS:-192.168.71.1}
OURS=${OURS:-192.168.71.2}
PORT=${PORT:-9000}
# 256 MiB, so a run is SECONDS. At the first measured rate that is
# about ten seconds -- long enough that the handshake and the process
# launch are noise rather than the measurement.
BYTES=${BYTES:-268435456}
REPS=${REPS:-3}
HERE=$(cd "$(dirname "$0")" && pwd)

PEER_PID=""

cleanup() {
    status=$?
    [ -n "$PEER_PID" ] && kill "$PEER_PID" 2>/dev/null || true
    ip link set "$TAP" down 2>/dev/null || true
    ip tuntap del dev "$TAP" mode tap 2>/dev/null || true
    rm -f "$HERE/.peer.py" "$HERE"/.peer.*.out "$HERE/.rates" 2>/dev/null || true
    exit $status
}
trap cleanup EXIT INT TERM

[ "$(id -u)" = "0" ] || { echo "needs root" >&2; exit 2; }
[ -e /dev/net/tun ] || { echo "no /dev/net/tun" >&2; exit 2; }

ip tuntap del dev "$TAP" mode tap 2>/dev/null || true
ip tuntap add dev "$TAP" mode tap
ip addr add "$THEIRS/24" dev "$TAP"
ip link set "$TAP" up

# The peer runs on the KERNEL's stack. It is deliberately dull: accept,
# then either count what arrives or push a fixed number of bytes, then
# report the count so work parity can be checked rather than assumed.
cat > "$HERE/.peer.py" <<'PY'
import socket, sys

host, port, mode, want = sys.argv[1], int(sys.argv[2]), sys.argv[3], int(sys.argv[4])
srv = socket.socket()
srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
srv.bind((host, port))
srv.listen(1)
# Announce readiness rather than being probed: this socket accepts ONCE,
# so a probe that connects to check would consume the accept the real
# client needs.
print("listening", flush=True)
conn, _ = srv.accept()
conn.settimeout(120)

total = 0
if mode == "sink":
    while True:
        try:
            block = conn.recv(1 << 16)
        except OSError:
            break
        if not block:
            break
        total += len(block)
else:
    block = b"\x5a" * (1 << 15)
    while total < want:
        n = conn.send(block[: min(len(block), want - total)])
        if n <= 0:
            break
        total += n
    conn.shutdown(socket.SHUT_WR)

print("peer-bytes=%d" % total, flush=True)
conn.close()
srv.close()
PY

run_one() {
    direction=$1
    peer_mode=$2
    echo "== $direction, $REPS reps of $BYTES bytes =="
    rates=""
    rep=1
    while [ "$rep" -le "$REPS" ]; do
        # A file PER REP. Sharing one is a race: the shell forks and the
        # CHILD truncates, so the readiness check below can still see the
        # previous rep's `listening` line and charge ahead before python
        # has bound. That is what made rep 2 "unreachable".
        out_file="$HERE/.peer.$rep.out"
        rm -f "$out_file"
        python3 "$HERE/.peer.py" "$THEIRS" "$PORT" "$peer_mode" "$BYTES" > "$out_file" 2>&1 &
        PEER_PID=$!
        # Wait for the peer's own `listening` line. The first version
        # guessed a sleep and a rep failed to connect; the second probed
        # by connecting and ATE the single accept the client needed.
        w=0
        while [ $w -lt 100 ]; do
            grep -q '^listening$' "$out_file" 2>/dev/null && break
            w=$((w + 1)); sleep 0.05
        done
        [ $w -lt 100 ] || { echo "  peer never announced itself" >&2; exit 8; }
        out=$(./target/debug/throughput "$TAP" "$OURS" "$THEIRS" "$PORT" "$direction" "$BYTES")
        wait "$PEER_PID" 2>/dev/null || true
        PEER_PID=""

        # Work parity, every rep: a rate computed over a transfer that
        # silently truncated is the classic fast wrong answer.
        peer=$(sed -n 's/peer-bytes=//p' "$out_file" | head -1)
        if [ "$peer" != "$BYTES" ]; then
            echo "  WORK PARITY FAILED on rep $rep: asked $BYTES, peer saw $peer" >&2
            exit 7
        fi

        secs=$(echo "$out" | sed -n 's/.*seconds=\([0-9.]*\).*/\1/p')
        rate=$(echo "$out" | sed -n 's/.*throughput=\([0-9.]*\) MiB.*/\1/p')
        echo "  rep $rep: ${secs}s  ${rate} MiB/s  (peer confirms $peer bytes)"
        rates="$rates $rate"
        rm -f "$out_file"
        rep=$((rep + 1))
        PORT=$((PORT + 1))
    done

    # Best of N, with the spread beside it. Best-of-N finds the floor the
    # machine can actually reach; the spread says how much to trust it.
    echo "$rates" | tr ' ' '\n' | grep -v '^$' | sort -n > "$HERE/.rates"
    lo=$(head -1 "$HERE/.rates")
    hi=$(tail -1 "$HERE/.rates")
    echo "  BEST ${hi} MiB/s, worst ${lo} MiB/s"
    rm -f "$HERE/.rates"
    echo
}

cd "$HERE"
cargo build --quiet --bin throughput --features tap

echo "rig: $TAP, kernel side $THEIRS, our side $OURS, $BYTES bytes per direction"
echo
run_one tx sink
run_one rx source
echo "METHOD: best of $REPS, $BYTES bytes per rep, work parity checked every rep,"
echo "        CLOCK_MONOTONIC, timer stops when the send buffer DRAINS (not when"
echo "        the copy returns). A userspace poll loop over a TAP on a"
echo "        workstation: this bounds nothing about a chip."

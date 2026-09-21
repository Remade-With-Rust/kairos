#!/bin/sh
# INTEROP: our stack against the LINUX KERNEL's.
#
# Every other test in this crate has ours on both ends, which proves the
# Kairos packages compose with each other and nothing about whether they
# compose with anybody else. This one puts a foreign stack on the far side
# of a TAP device and makes a real HTTP request across it.
#
#     sudo sh tools/vertical/interop.sh
#
# It needs root and /dev/net/tun, which is why it is a script and a binary
# rather than a `cargo test`: an ordinary test run must need neither. On
# this workstation it runs under WSL, where both are available.
#
# Everything it creates it removes, including on failure.
set -eu

TAP=${TAP:-kairos0}
THEIRS=${THEIRS:-192.168.69.1}
OURS=${OURS:-192.168.69.2}
PORT=${PORT:-8080}
HERE=$(cd "$(dirname "$0")" && pwd)

SERVER_PID=""

cleanup() {
    status=$?
    [ -n "$SERVER_PID" ] && kill "$SERVER_PID" 2>/dev/null || true
    ip link set "$TAP" down 2>/dev/null || true
    ip tuntap del dev "$TAP" mode tap 2>/dev/null || true
    rm -rf "$HERE/.interop" 2>/dev/null || true
    exit $status
}
trap cleanup EXIT INT TERM

[ "$(id -u)" = "0" ] || { echo "needs root: a TAP device is not an unprivileged thing" >&2; exit 2; }
[ -e /dev/net/tun ] || { echo "no /dev/net/tun" >&2; exit 2; }

echo "== the rig =="
ip tuntap del dev "$TAP" mode tap 2>/dev/null || true
ip tuntap add dev "$TAP" mode tap
ip addr add "$THEIRS/24" dev "$TAP"
ip link set "$TAP" up
echo "  $TAP up, kernel side $THEIRS, our side $OURS"

# A stock server, on the kernel's stack. Nothing about it knows what is on
# the other side of the TAP, which is the entire point.
mkdir -p "$HERE/.interop"
printf 'hello from a foreign stack\n' > "$HERE/.interop/index.html"
( cd "$HERE/.interop" && python3 -m http.server "$PORT" --bind "$THEIRS" >/dev/null 2>&1 ) &
SERVER_PID=$!
echo "  python3 http.server on $THEIRS:$PORT (pid $SERVER_PID)"

# Give it a moment to bind, then check it really is listening before
# blaming our stack for anything.
i=0
while [ $i -lt 50 ]; do
    if python3 - "$THEIRS" "$PORT" <<'PY' 2>/dev/null
import socket, sys
s = socket.socket()
s.settimeout(0.2)
try:
    s.connect((sys.argv[1], int(sys.argv[2])))
except OSError:
    sys.exit(1)
sys.exit(0)
PY
    then
        break
    fi
    i=$((i + 1))
    sleep 0.1
done
[ $i -lt 50 ] || { echo "the server never came up; not our stack's fault" >&2; exit 3; }
echo "  the server answers the kernel's own loopback, so it is really there"

echo
echo "== our stack =="
cd "$HERE"
cargo build --quiet --bin interop --features tap
./target/debug/interop "$TAP" "$OURS" "$THEIRS" "$PORT"

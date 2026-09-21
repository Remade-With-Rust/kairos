#!/bin/sh
# INTEROP: our MQTT client against a FOREIGN broker, over our own TCP.
#
# The mission plan's K7 kill test is "a Kairos device publishing over our
# TCP to the Home Computer's rumqttd, one hour, zero lost keep-alives".
# This is that test with two substitutions, and both are stated in the
# output rather than hidden:
#
#   * the broker is MOSQUITTO, not rumqttd. Both are third party and
#     neither has heard of us, which is what the test is actually asking.
#   * the duration is seconds, not an hour. The hour is a SOAK and needs a
#     deployment; what a workstation settles is whether keep-alives are
#     exchanged correctly at all.
#
#     sudo sh tools/vertical/mqtt-interop.sh
#
# Needs root and /dev/net/tun. Everything it creates it removes.
set -eu

TAP=${TAP:-kairos1}
THEIRS=${THEIRS:-192.168.70.1}
OURS=${OURS:-192.168.70.2}
PORT=${PORT:-1883}
SECONDS_HELD=${SECONDS_HELD:-8}
HERE=$(cd "$(dirname "$0")" && pwd)

BROKER_PID=""

cleanup() {
    status=$?
    [ -n "$BROKER_PID" ] && kill "$BROKER_PID" 2>/dev/null || true
    ip link set "$TAP" down 2>/dev/null || true
    ip tuntap del dev "$TAP" mode tap 2>/dev/null || true
    rm -f "$HERE/.mosquitto.conf" 2>/dev/null || true
    exit $status
}
trap cleanup EXIT INT TERM

[ "$(id -u)" = "0" ] || { echo "needs root: a TAP device is not an unprivileged thing" >&2; exit 2; }
[ -e /dev/net/tun ] || { echo "no /dev/net/tun" >&2; exit 2; }
command -v mosquitto >/dev/null || { echo "no mosquitto; apt-get install mosquitto" >&2; exit 2; }

echo "== the rig =="
ip tuntap del dev "$TAP" mode tap 2>/dev/null || true
ip tuntap add dev "$TAP" mode tap
ip addr add "$THEIRS/24" dev "$TAP"
ip link set "$TAP" up
echo "  $TAP up, broker side $THEIRS, our side $OURS"

cat > "$HERE/.mosquitto.conf" <<EOF
listener $PORT $THEIRS
allow_anonymous true
EOF

mosquitto -c "$HERE/.mosquitto.conf" >/dev/null 2>&1 &
BROKER_PID=$!
echo "  mosquitto $(mosquitto -h 2>&1 | head -1 | awk '{print $3}') on $THEIRS:$PORT (pid $BROKER_PID)"

i=0
while [ $i -lt 50 ]; do
    if python3 - "$THEIRS" "$PORT" <<'PY' 2>/dev/null
import socket, sys
s = socket.socket(); s.settimeout(0.2)
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
[ $i -lt 50 ] || { echo "the broker never came up; not our stack's fault" >&2; exit 3; }
echo "  it answers on the kernel's own stack, so it is really there"

echo
echo "== our stack =="
cd "$HERE"
cargo build --quiet --bin mqtt_interop --features tap
./target/debug/mqtt_interop "$TAP" "$OURS" "$THEIRS" "$PORT" "$SECONDS_HELD" mosquitto

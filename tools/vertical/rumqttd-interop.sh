#!/bin/sh
# THE KILL TEST'S NAMED BROKER: our MQTT client against rumqttd.
#
# The mission plan asks for "a Kairos device publishing over our TCP to the
# Home Computer's rumqttd, one hour, zero lost keep-alives". This is that,
# with the broker no longer substituted:
#
#     sudo SECONDS_HELD=3600 sh tools/vertical/rumqttd-interop.sh
#
# rumqttd is reached on its **v5** listener, because this client speaks
# MQTT 5 and rumqttd's v4 and v5 listeners are different ports. Sending a
# v5 CONNECT at a v4 listener is a protocol-version refusal, not a stack
# problem, and that is the kind of thing a rig should get right before it
# blames anybody.
#
# Needs root and /dev/net/tun. Everything it creates it removes.
set -eu

TAP=${TAP:-kairos3}
THEIRS=${THEIRS:-192.168.72.1}
OURS=${OURS:-192.168.72.2}
PORT=${PORT:-1884}
SECONDS_HELD=${SECONDS_HELD:-30}
HERE=$(cd "$(dirname "$0")" && pwd)

BROKER_PID=""

cleanup() {
    status=$?
    [ -n "$BROKER_PID" ] && kill "$BROKER_PID" 2>/dev/null || true
    ip link set "$TAP" down 2>/dev/null || true
    ip tuntap del dev "$TAP" mode tap 2>/dev/null || true
    rm -f "$HERE/.rumqttd.toml" "$HERE/.rumqttd.log" 2>/dev/null || true
    exit $status
}
trap cleanup EXIT INT TERM

[ "$(id -u)" = "0" ] || { echo "needs root" >&2; exit 2; }
[ -e /dev/net/tun ] || { echo "no /dev/net/tun" >&2; exit 2; }
command -v rumqttd >/dev/null || {
    echo "no rumqttd. NOTE: \`cargo install rumqttd --locked\` FAILS -- its" >&2
    echo "pinned \`metrics\` does not compile (E0521). Install WITHOUT --locked." >&2
    exit 2
}

echo "== the rig =="
ip tuntap del dev "$TAP" mode tap 2>/dev/null || true
ip tuntap add dev "$TAP" mode tap
ip addr add "$THEIRS/24" dev "$TAP"
ip link set "$TAP" up
echo "  $TAP up, broker side $THEIRS, our side $OURS"

# The smallest config with a v5 listener where we want it.
cat > "$HERE/.rumqttd.toml" <<EOF
id = 0

[router]
id = 0
max_connections = 64
max_outgoing_packet_count = 200
max_segment_size = 1048576
max_segment_count = 4

[v5.1]
name = "v5-1"
listen = "$THEIRS:$PORT"
next_connection_delay_ms = 1
    [v5.1.connections]
    connection_timeout_ms = 60000
    max_payload_size = 20480
    max_inflight_count = 100
    dynamic_filters = true
EOF

rumqttd -c "$HERE/.rumqttd.toml" -q > "$HERE/.rumqttd.log" 2>&1 &
BROKER_PID=$!
echo "  rumqttd $(rumqttd --version 2>&1 | awk '{print $2}') on $THEIRS:$PORT (v5 listener, pid $BROKER_PID)"

i=0
while [ $i -lt 60 ]; do
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
[ $i -lt 60 ] || { echo "rumqttd never came up; not our stack's fault" >&2; cat "$HERE/.rumqttd.log" >&2; exit 3; }
echo "  it answers on the kernel's own stack, so it is really there"

echo
echo "== our stack =="
cd "$HERE"
cargo build --quiet --bin mqtt_interop --features tap
./target/debug/mqtt_interop "$TAP" "$OURS" "$THEIRS" "$PORT" "$SECONDS_HELD" rumqttd

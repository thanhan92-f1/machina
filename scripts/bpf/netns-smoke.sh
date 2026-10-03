#!/usr/bin/env bash
# Netns smoke test for machina-bpfd: a veth pair stands in for a VM tap.
# Runs bpfd on a private socket/state dir with auto-attach limited to the test
# veth, so real VM taps on the host are never touched.
#
#   sudo ./scripts/bpf/netns-smoke.sh [path/to/machina-bpfd]
set -euo pipefail

BPFD=${1:-./target/debug/machina-bpfd}
NS=mnbpf-smoke
HOST_IF=mnt-smoke0
PEER_IF=mnt-smoke0p
HOST_IP=10.199.77.1
PEER_IP=10.199.77.2
WORK=$(mktemp -d /tmp/mnbpf-smoke.XXXX)
SOCK=$WORK/bpfd.sock
PASS=0
FAIL=0

cleanup() {
  [[ -n "${BPFD_PID:-}" ]] && kill "$BPFD_PID" 2>/dev/null && wait "$BPFD_PID" 2>/dev/null || true
  [[ -n "${HTTP_PID:-}" ]] && kill "$HTTP_PID" 2>/dev/null || true
  ip netns del "$NS" 2>/dev/null || true
  ip link del "$HOST_IF" 2>/dev/null || true
}
trap cleanup EXIT

req() {
  python3 - "$SOCK" "$1" <<'PY'
import json, socket, sys
s = socket.socket(socket.AF_UNIX)
s.connect(sys.argv[1])
s.sendall(sys.argv[2].encode() + b"\n")
buf = b""
while not buf.endswith(b"\n"):
    c = s.recv(65536)
    if not c:
        break
    buf += c
print(buf.decode().strip())
PY
}

jq_ok() { python3 -c 'import json,sys; d=json.load(sys.stdin); sys.exit(0 if d.get("ok") else 1)'; }
must() {
  local out
  out=$(cat)
  if ! jq_ok <<<"$out"; then
    echo "WARN  request failed: $out"
  fi
}
policy() { req "{\"op\":\"apply_policy\",\"policy\":{\"id\":\"$1\",\"kind\":\"$2\",\"match\":\"$3\"}}" | must; }
unpolicy() { req "{\"op\":\"remove_policy\",\"id\":\"$1\"}" | must; }
enforce() { req '{"op":"set_mode","mode":"enforce","lease_secs":60}' | must; }
observe() { req '{"op":"set_mode","mode":"observe"}' | must; }

check() {
  local name=$1
  shift
  if "$@"; then
    echo "PASS  $name"
    PASS=$((PASS + 1))
  else
    echo "FAIL  $name"
    FAIL=$((FAIL + 1))
  fi
}

ping_ok() { ip netns exec "$NS" ping -c1 -W1 "$HOST_IP" >/dev/null 2>&1; }
ping_blocked() { ! ping_ok; }
http_ok() { ip netns exec "$NS" curl -s -m2 -o /dev/null "http://$HOST_IP:18080/"; }

ip netns add "$NS"
ip link add "$HOST_IF" type veth peer name "$PEER_IF"
ip link set "$PEER_IF" netns "$NS"
ip addr add "$HOST_IP/30" dev "$HOST_IF"
ip link set "$HOST_IF" up
ip netns exec "$NS" ip addr add "$PEER_IP/30" dev "$PEER_IF"
ip netns exec "$NS" ip link set "$PEER_IF" up
ip netns exec "$NS" ip link set lo up

python3 -m http.server 18080 --bind "$HOST_IP" >/dev/null 2>&1 &
HTTP_PID=$!

cat >"$WORK/bpfd-state.json" <<EOF
{"telemetry": {"exec": true, "connect": true, "flows": true, "dns": true,
               "file_watch": ["/etc/shadow"], "iface_patterns": ["$HOST_IF"]}}
EOF

RUST_LOG=${RUST_LOG:-info} "$BPFD" --socket "$SOCK" --state-dir "$WORK" --socket-group "" \
  >"$WORK/bpfd.log" 2>&1 &
BPFD_PID=$!
for _ in $(seq 50); do [[ -S "$SOCK" ]] && break; sleep 0.2; done
if [[ ! -S "$SOCK" ]]; then
  echo "bpfd failed to start:"
  cat "$WORK/bpfd.log"
  exit 1
fi

check "status ok" bash -c "$(declare -f req jq_ok); SOCK=$SOCK; req '{\"op\":\"status\"}' | jq_ok"
check "test veth auto-attached" bash -c "$(declare -f req); SOCK=$SOCK; req '{\"op\":\"list_interfaces\"}' | grep -q $HOST_IF"
check "baseline ping" ping_ok
check "baseline http" http_ok
sleep 1
check "flows recorded" bash -c "$(declare -f req); SOCK=$SOCK; req '{\"op\":\"flows\"}' | grep -q $PEER_IP"

req "{\"op\":\"apply_policy\",\"policy\":{\"id\":\"smoke-deny\",\"name\":\"smoke\",\"kind\":\"deny_ip\",\"match\":\"$HOST_IP/32\",\"enabled\":true}}" | must
check "observe: deny_ip does not block" ping_ok
sleep 0.5
check "observe: deny event recorded" bash -c "$(declare -f req); SOCK=$SOCK; req '{\"op\":\"events\",\"kind\":\"deny\"}' | grep -q observed"

req '{"op":"set_mode","mode":"enforce","lease_secs":60}' | must
check "enforce: deny_ip blocks ping" ping_blocked
req '{"op":"set_mode","mode":"observe"}' | must
check "observe again: ping restored" ping_ok
req '{"op":"remove_policy","id":"smoke-deny"}' | must

req "{\"op\":\"apply_policy\",\"policy\":{\"id\":\"smoke-allow\",\"name\":\"smoke\",\"kind\":\"tc_allow\",\"match\":\"$HOST_IP:18080/tcp\",\"enabled\":true}}" | must
req '{"op":"set_mode","mode":"enforce","lease_secs":60}' | must
check "enforce allowlist: allowed http passes" http_ok
check "enforce allowlist: icmp blocked" ping_blocked
req '{"op":"set_mode","mode":"observe"}' | must
req '{"op":"remove_policy","id":"smoke-allow"}' | must
check "allowlist removed: ping restored" ping_ok

req '{"op":"capture_start","iface":"'"$HOST_IF"'","duration_secs":3}' | must
ip netns exec "$NS" ping -c3 -i0.2 "$HOST_IP" >/dev/null 2>&1 || true
sleep 4
check "capture has packets" bash -c "$(declare -f req); SOCK=$SOCK; req '{\"op\":\"capture_list\"}' | python3 -c 'import json,sys; d=json.load(sys.stdin)[\"data\"]; sys.exit(0 if d and d[0][\"packets\"]>0 else 1)'"

cat /etc/hostname >/dev/null
check "exec events recorded" bash -c "$(declare -f req); SOCK=$SOCK; req '{\"op\":\"proc_events\",\"kind\":\"exec\"}' | grep -q '\"exec\"'"
check "net health readable" bash -c "$(declare -f req jq_ok); SOCK=$SOCK; req '{\"op\":\"net_health\"}' | jq_ok"


# Process / file / port enforcement (SIGKILL / drop only while the lease is active).
cp /bin/true "$WORK/mn-forbidden"
echo secret >"$WORK/secret"
policy smoke-exec deny_process "$WORK/mn-forbidden"
policy smoke-file deny_file "$WORK/secret"
policy smoke-port deny_port "18080/tcp"
check "observe: denied exec still runs" "$WORK/mn-forbidden"
check "observe: denied file still readable" bash -c "cat '$WORK/secret' >/dev/null"
check "observe: denied port still reachable" http_ok
enforce
check "enforce: denied exec killed" bash -c "! '$WORK/mn-forbidden'"
check "enforce: denied file read killed" bash -c "! cat '$WORK/secret' >/dev/null 2>&1"
check "enforce: denied port blocked" bash -c "$(declare -f http_ok); NS=$NS HOST_IP=$HOST_IP; ! http_ok"
check "enforce: other exec unaffected" /bin/true
observe
check "observe again: exec restored" "$WORK/mn-forbidden"
sleep 0.5
check "exec deny event attributed" bash -c "$(declare -f req); SOCK=$SOCK; req '{\"op\":\"proc_events\",\"kind\":\"exec\",\"limit\":5000}' | grep -q smoke-exec"
unpolicy smoke-exec
unpolicy smoke-file
unpolicy smoke-port

# QoS: cap traffic towards the workload at 8 Mbit/s and time a 3 MB download.
head -c 3000000 /dev/urandom >"$WORK/blob"
(cd "$WORK" && python3 -m http.server 18081 --bind "$HOST_IP" >/dev/null 2>&1) &
BLOB_PID=$!
sleep 0.5
req '{"op":"set_qos","iface":"'"$HOST_IF"'","ingress_bps":8000000}' | must
start=$(date +%s.%N)
ip netns exec "$NS" curl -s -m20 -o /dev/null "http://$HOST_IP:18081/blob" || true
secs=$(echo "$(date +%s.%N) - $start" | bc)
echo "      3 MB at 8 Mbit/s took ${secs}s"
check "qos paces download (>= 2s)" bash -c "(( \$(echo '$secs >= 2' | bc) ))"
req '{"op":"set_qos","iface":"'"$HOST_IF"'","ingress_bps":0}' | must
kill $BLOB_PID 2>/dev/null || true

echo
echo "passed=$PASS failed=$FAIL  (log: $WORK/bpfd.log)"
grep -E "WARN|ERROR" "$WORK/bpfd.log" | head -20 || true
if [[ $FAIL -ne 0 ]]; then
  echo "--- interfaces"; req '{"op":"list_interfaces"}'
fi
[[ $FAIL -eq 0 ]]

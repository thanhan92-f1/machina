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

# Connection rate limiting: 2 new connections/s, burst 2. The opening SYN of
# each excess connection is dropped while enforcing (observed otherwise).
http_burst() {
  local ok=0
  for _ in $(seq 10); do
    ip netns exec "$NS" curl -s -m0.5 -o /dev/null "http://$HOST_IP:18080/" && ok=$((ok + 1))
  done
  echo "$ok"
}
policy smoke-rate rate_limit "2/s burst 2"
n=$(http_burst)
echo "      observe: $n/10 connections"
check "observe: rate_limit does not block" test "$n" -eq 10
check "observe: rate_limit counted" bash -c "$(declare -f req); SOCK=$SOCK; req '{\"op\":\"status\"}' | python3 -c 'import json,sys; sys.exit(0 if json.load(sys.stdin)[\"data\"][\"counters\"][\"rate_limited\"]>0 else 1)'"
enforce
n=$(http_burst)
echo "      enforce: $n/10 connections"
check "enforce: rate_limit drops excess connections" bash -c "(( $n >= 1 && $n <= 6 ))"
observe
unpolicy smoke-rate
sleep 1
n=$(http_burst)
check "rate_limit removed: http restored" test "$n" -eq 10

# L7 visibility: HTTP request line + Host, TLS ClientHello SNI/ALPN.
python3 - "$HOST_IP" <<'PY' >/dev/null 2>&1 &
import socket, sys, time
s = socket.socket()
s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind((sys.argv[1], 18443))
s.listen(8)
end = time.time() + 30
while time.time() < end:
    c, _ = s.accept()
    c.recv(4096)
    c.close()
PY
TLS_PID=$!
sleep 0.5
ip netns exec "$NS" curl -s -m2 -o /dev/null -A mn-smoke/1 "http://$HOST_IP:18080/l7-probe" || true
ip netns exec "$NS" python3 - "$HOST_IP" <<'PY' >/dev/null 2>&1 || true
import socket, ssl, sys
ctx = ssl.create_default_context()
ctx.set_alpn_protocols(["h2", "http/1.1"])
ctx.check_hostname = False
ctx.verify_mode = ssl.CERT_NONE
s = socket.create_connection((sys.argv[1], 18443), timeout=2)
try:
    ctx.wrap_socket(s, server_hostname="smoke.machina.test")
except Exception:
    pass
PY
sleep 1
kill $TLS_PID 2>/dev/null || true
l7() { req '{"op":"l7","limit":200}'; }
check "l7: http request recorded" bash -c "$(declare -f req l7); SOCK=$SOCK; l7 | python3 -c 'import json,sys; d=json.load(sys.stdin)[\"data\"]; sys.exit(0 if any(r.get(\"path\")==\"/l7-probe\" and r.get(\"method\")==\"GET\" and r.get(\"user_agent\")==\"mn-smoke/1\" for r in d) else 1)'"
check "l7: tls sni + alpn recorded" bash -c "$(declare -f req l7); SOCK=$SOCK; l7 | python3 -c 'import json,sys; d=json.load(sys.stdin)[\"data\"]; sys.exit(0 if any(r.get(\"host\")==\"smoke.machina.test\" and \"h2\" in (r.get(\"alpn\") or []) for r in d) else 1)'"

# Per-workload accounting: the test veth has no VM, so it is keyed by iface.
acct() { req "{\"op\":\"accounting\",\"vm\":\"iface:$HOST_IF\"}"; }
check "accounting: bytes counted both ways" bash -c "$(declare -f req acct); SOCK=$SOCK HOST_IF=$HOST_IF; acct | python3 -c 'import json,sys; d=json.load(sys.stdin)[\"data\"]; sys.exit(0 if d and d[0][\"tx_bytes\"]>0 and d[0][\"rx_bytes\"]>0 and d[0][\"rx_pkts\"]>0 else 1)'"
check "accounting: enforcement drops counted" bash -c "$(declare -f req acct); SOCK=$SOCK HOST_IF=$HOST_IF; acct | python3 -c 'import json,sys; sys.exit(0 if json.load(sys.stdin)[\"data\"][0][\"drops\"]>0 else 1)'"
req '{"op":"reset_accounting"}' | must
check "accounting: reset zeroes totals" bash -c "$(declare -f req acct); SOCK=$SOCK HOST_IF=$HOST_IF; acct | python3 -c 'import json,sys; d=json.load(sys.stdin)[\"data\"]; sys.exit(0 if d[0][\"tx_bytes\"] < 2000 else 1)'"

# deny_dns: answers for *.blocked.test feed their A records into the deny set.
DNS_TARGET=10.199.77.5
ip addr add "$DNS_TARGET/32" dev "$HOST_IF"
ip netns exec "$NS" ip route add "$DNS_TARGET/32" dev "$PEER_IF"
python3 - "$HOST_IP" "$DNS_TARGET" <<'PY' >/dev/null 2>&1 &
import socket, sys, time
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.bind((sys.argv[1], 53))
ip = socket.inet_aton(sys.argv[2])
end = time.time() + 30
while time.time() < end:
    q, a = s.recvfrom(512)
    end_q = 12
    while q[end_q] != 0:
        end_q += q[end_q] + 1
    question = q[12:end_q + 5]
    ans = b"\xc0\x0c\x00\x01\x00\x01\x00\x00\x00\x3c\x00\x04" + ip
    s.sendto(q[:2] + b"\x81\x80\x00\x01\x00\x01\x00\x00\x00\x00" + question + ans, a)
PY
DNS_PID=$!
sleep 0.5
dns_query() {
  ip netns exec "$NS" python3 - "$HOST_IP" "$1" <<'PY'
import socket, sys
q = b"\x12\x34\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00"
q += b"".join(bytes([len(p)]) + p.encode() for p in sys.argv[2].split(".")) + b"\x00\x00\x01\x00\x01"
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.settimeout(2)
s.sendto(q, (sys.argv[1], 53))
s.recv(512)
PY
}
ping_target() { ip netns exec "$NS" ping -c1 -W1 "$DNS_TARGET" >/dev/null 2>&1; }
check "dns: baseline target reachable" ping_target
policy smoke-dns deny_dns "*.blocked.test"
enforce
dns_query www.blocked.test || true
sleep 1
check "dns: query recorded" bash -c "$(declare -f req); SOCK=$SOCK; req '{\"op\":\"dns\",\"limit\":200}' | grep -q www.blocked.test"
check "enforce: deny_dns blocks the resolved address" bash -c "$(declare -f ping_target); NS=$NS DNS_TARGET=$DNS_TARGET; ! ping_target"
check "enforce: deny_dns leaves other hosts alone" ping_ok
observe
unpolicy smoke-dns
check "deny_dns removed: target restored" ping_target
kill $DNS_PID 2>/dev/null || true

# XDP shield on the test veth (host side = traffic from the netns): per-source
# ICMP bucket of 5 pps, then deny/allow CIDRs. Drops only under the lease.
shield() { req "{\"op\":\"shield_configure\",\"config\":{\"iface\":\"$HOST_IF\",\"protected\":[\"$HOST_IP\"],\"icmp_pps\":5,\"burst_secs\":1,$1}}" | must; }
shjs() { req '{"op":"shield_status"}' | python3 -c "import json,sys; d=json.load(sys.stdin)['data']; print($1)"; }
flood() { ip netns exec "$NS" ping -c40 -i0.01 -W1 -q "$HOST_IP" 2>/dev/null | sed -n 's/.* \([0-9]*\) received.*/\1/p'; }
xdp_on() { ip -d link show dev "$HOST_IF" | grep -q 'prog/xdp'; }
shield '"mode":"audit"'
check "shield: dispatcher attached to test veth" xdp_on
n=$(flood)
echo "      shield audit: $n/40 replies"
check "shield audit: flood not dropped" test "${n:-0}" -eq 40
check "shield audit: over-rate audited" test "$(shjs "d['stats']['audited']")" -gt 0
check "shield: top source is the netns" bash -c "$(declare -f req shjs); SOCK=$SOCK; shjs \"[s['addr']+'/'+s['class'] for s in d['sources']]\" | grep -q '$PEER_IP/icmp'"
shield '"mode":"enforce"'
check "shield enforce without lease: not enforcing" test "$(shjs "d['enforcing']")" = False
enforce
check "shield enforce: status enforcing" test "$(shjs "d['enforcing']")" = True
sleep 1
n=$(flood)
echo "      shield enforce: $n/40 replies"
check "shield enforce: flood rate-limited" bash -c "(( ${n:-40} >= 1 && ${n:-40} <= 20 ))"
check "shield enforce: drops counted" test "$(shjs "d['stats']['dropped']")" -gt 0
shield "\"mode\":\"enforce\",\"deny\":[\"$PEER_IP/32\"]"
sleep 1
check "shield enforce: deny CIDR blocks source" ping_blocked
shield "\"mode\":\"enforce\",\"allow\":[\"$PEER_IP/32\"]"
n=$(flood)
check "shield enforce: allow CIDR bypasses limits" test "${n:-0}" -eq 40
observe
shield '"mode":"off"'
check "shield off: dispatcher detached" bash -c "$(declare -f xdp_on); HOST_IF=$HOST_IF; ! xdp_on"
check "shield off: ping restored" ping_ok

echo
echo "passed=$PASS failed=$FAIL  (log: $WORK/bpfd.log)"
grep -E "WARN|ERROR" "$WORK/bpfd.log" | head -20 || true
if [[ $FAIL -ne 0 ]]; then
  echo "--- interfaces"; req '{"op":"list_interfaces"}'
fi
[[ $FAIL -eq 0 ]]

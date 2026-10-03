#!/usr/bin/env bash
# machina-cni smoke: the real CNI plugin wires two netns "pods" against a
# private machina-bpfd, then checks routing, same-node redirect,
# NetworkPolicy and socket-LB services. No uplink is configured (NodePort is
# never attached to a real NIC) and an existing cluster CNI is left alone:
# only a test subnet and a test service VIP are used.
#
#   sudo ./scripts/bpf/cni-smoke.sh [dir with machina-bpfd + machina-cni]
set -euo pipefail

BIN=$(cd "${1:-./target/release}" && pwd)
WORK=$(mktemp -d /tmp/mncni-smoke.XXXX)
export MACHINA_BPFD_SOCK=$WORK/bpfd.sock
SOCK=$MACHINA_BPFD_SOCK
SUBNET=10.199.90.0/24
VIP=10.199.88.10
NS_A=mncni-a
NS_B=mncni-b
PASS=0
FAIL=0

cleanup() {
  for ns in "$NS_A" "$NS_B"; do
    [[ -e /var/run/netns/$ns ]] && cni DEL "$ns" >/dev/null 2>&1 || true
    ip netns del "$ns" 2>/dev/null || true
  done
  [[ -n "${BPFD_PID:-}" ]] && kill "$BPFD_PID" 2>/dev/null && wait "$BPFD_PID" 2>/dev/null || true
}
trap cleanup EXIT

req() {
  python3 - "$SOCK" "$1" <<'PY'
import socket, sys
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
must() {
  local out
  out=$(cat)
  python3 -c 'import json,sys; sys.exit(0 if json.loads(sys.argv[1]).get("ok") else 1)' "$out" \
    || echo "WARN  request failed: $out"
}
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

cni() {
  printf '{"cniVersion":"1.0.0","name":"smoke","type":"machina-cni","subnet":"%s","ipam_dir":"%s"}' \
    "$SUBNET" "$WORK/ipam" |
    CNI_COMMAND=$1 CNI_CONTAINERID="smoke-$2" CNI_NETNS=/var/run/netns/$2 CNI_IFNAME=eth0 \
      CNI_PATH="$BIN" CNI_ARGS="K8S_POD_NAMESPACE=smoke;K8S_POD_NAME=$2" "$BIN/machina-cni"
}
pod_ip() { python3 -c 'import json,sys; print(json.load(sys.stdin)["ips"][0]["address"].split("/")[0])'; }

cat >"$WORK/bpfd-state.json" <<EOF
{"telemetry": {"exec": false, "connect": false, "flows": false, "dns": false,
               "iface_patterns": ["mncni-none"]}}
EOF
RUST_LOG=${RUST_LOG:-info} "$BIN/machina-bpfd" --socket "$SOCK" --state-dir "$WORK" --socket-group "" \
  >"$WORK/bpfd.log" 2>&1 &
BPFD_PID=$!
for _ in $(seq 50); do [[ -S "$SOCK" ]] && break; sleep 0.2; done
[[ -S "$SOCK" ]] || { echo "bpfd failed to start:"; cat "$WORK/bpfd.log"; exit 1; }

ip netns add "$NS_A"
ip netns add "$NS_B"
RES_A=$(cni ADD "$NS_A")
RES_B=$(cni ADD "$NS_B")
A=$(pod_ip <<<"$RES_A")
B=$(pod_ip <<<"$RES_B")
echo "      pod a=$A pod b=$B"
check "plugin ADD assigns addresses" test -n "$A" -a -n "$B" -a "$A" != "$B"
check "pod has default route via link-local gateway" bash -c "ip netns exec $NS_A ip route | grep -q 'default via 169.254.1.1'"

NODE_ADDR=$(ip -4 route get "$A" | sed -n 's/.* src \([0-9.]*\).*/\1/p')
req "{\"op\":\"cni_configure\",\"node_addr\":\"$NODE_ADDR\",\"uplink\":null}" | must
check "bpfd lists both endpoints" bash -c "$(declare -f req); SOCK=$SOCK; req '{\"op\":\"cni_status\"}' | python3 -c 'import json,sys; sys.exit(0 if len(json.load(sys.stdin)[\"data\"][\"endpoints\"])==2 else 1)'"

ip netns exec "$NS_B" python3 -m http.server 8080 --bind "$B" >/dev/null 2>&1 &
sleep 0.7

ping_ab() { ip netns exec "$NS_A" ping -c1 -W1 "$B" >/dev/null 2>&1; }
http_ab() { ip netns exec "$NS_A" curl -s -m2 -o /dev/null "http://$B:8080/"; }
ping_hb() { ping -c1 -W1 "$B" >/dev/null 2>&1; }
check "host -> pod ping" ping_hb
check "pod -> pod ping (bpf redirect)" ping_ab
check "pod -> pod http" http_ab

# bpfd reads one request per line.
cni_state() {
  req "$(tr -d '\n' <<EOF
{"op":"cni_sync","state":{"identities":[
{"ip":"$A","identity":1001,"ingress_isolated":false,"egress_isolated":${A_EGRESS:-false}},
{"ip":"$B","identity":1002,"ingress_isolated":${B_INGRESS:-false},"egress_isolated":false}],
"policy":[${POLICY:-}],"cidrs":[],"services":[${SERVICES:-}]}}
EOF
)" | must
}

B_INGRESS=true cni_state
check "default-deny ingress: pod -> pod http blocked" bash -c "$(declare -f http_ab); NS_A=$NS_A B=$B; ! http_ab"
check "default-deny ingress: pod -> pod ping blocked" bash -c "$(declare -f ping_ab); NS_A=$NS_A B=$B; ! ping_ab"
check "default-deny ingress: host (kubelet) still allowed" ping_hb

ALLOW_B='{"subject":1002,"peer":1001,"egress":false,"proto":6,"port":8080}'
B_INGRESS=true POLICY=$ALLOW_B cni_state
check "ingress allow tcp/8080: http passes" http_ab
check "ingress allow tcp/8080: ping still blocked" bash -c "$(declare -f ping_ab); NS_A=$NS_A B=$B; ! ping_ab"

A_EGRESS=true B_INGRESS=true POLICY=$ALLOW_B cni_state
check "default-deny egress on a: http blocked" bash -c "$(declare -f http_ab); NS_A=$NS_A B=$B; ! http_ab"
ALLOW_A='{"subject":1001,"peer":1002,"egress":true,"proto":6,"port":8080}'
A_EGRESS=true B_INGRESS=true POLICY="$ALLOW_B,$ALLOW_A" cni_state
check "egress + ingress allow: http passes" http_ab

SVC="{\"addr\":\"$VIP\",\"port\":80,\"proto\":6,\"backends\":[{\"addr\":\"$B\",\"port\":8080}],\"name\":\"smoke/web\"}"
B_INGRESS=true POLICY=$ALLOW_B SERVICES=$SVC cni_state
check "service VIP from host (socket LB)" curl -s -m2 -o /dev/null "http://$VIP/"
check "service VIP from pod (socket LB + policy)" ip netns exec "$NS_A" curl -s -m2 -o /dev/null "http://$VIP/"
B_INGRESS=true POLICY=$ALLOW_B cni_state
check "service removed: VIP unreachable" bash -c "! curl -s -m1 -o /dev/null http://$VIP/"

HOST_A=$(python3 -c 'import json,sys; print(json.load(sys.stdin)["interfaces"][0]["name"])' <<<"$RES_A")
cni DEL "$NS_A" >/dev/null
cni DEL "$NS_A" >/dev/null
check "DEL removes host veth (idempotent)" bash -c "! ip link show $HOST_A >/dev/null 2>&1"
check "DEL unregisters endpoint" bash -c "$(declare -f req); SOCK=$SOCK; req '{\"op\":\"cni_status\"}' | python3 -c 'import json,sys; sys.exit(0 if len(json.load(sys.stdin)[\"data\"][\"endpoints\"])==1 else 1)'"
check "DEL releases address" bash -c "! grep -rqs smoke-$NS_A $WORK/ipam"

echo
echo "passed=$PASS failed=$FAIL  (log: $WORK/bpfd.log)"
grep -E "WARN|ERROR" "$WORK/bpfd.log" | head -20 || true
if [[ $FAIL -ne 0 ]]; then
  echo "--- notes"; req '{"op":"status"}' | python3 -c 'import json,sys; print(json.load(sys.stdin)["data"]["notes"])'
fi
[[ $FAIL -eq 0 ]]

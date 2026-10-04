#!/usr/bin/env bash
# VM edge + QEMU sandbox smoke test for machina-bpfd.
#
# A veth pair stands in for a VM tap (named explicitly in the edge state, so
# libvirt discovery and real vnet/tap devices are never involved) and a
# throwaway cgroup stands in for a machine-qemu scope. bpfd runs on a private
# socket/state dir with auto-attach limited to the test veth.
#
#   sudo ./scripts/bpf/vm-edge-smoke.sh [path/to/machina-bpfd]
set -euo pipefail

BPFD=${1:-./target/debug/machina-bpfd}
NS=mnvme-smoke
HOST_IF=mnvme-a0
PEER_IF=mnvme-a0p
HOST_IP=10.199.81.1
VM_IP=10.199.81.2
VM=mnsmk-vm
CG_REL=mnsmk-sandbox
CG=/sys/fs/cgroup/$CG_REL
WORK=$(mktemp -d /tmp/mnvme-smoke.XXXX)
SOCK=$WORK/bpfd.sock
PASS=0
FAIL=0

cleanup() {
  [[ -n "${BPFD_PID:-}" ]] && kill "$BPFD_PID" 2>/dev/null && wait "$BPFD_PID" 2>/dev/null || true
  [[ -n "${HTTP_PID:-}" ]] && kill "$HTTP_PID" 2>/dev/null || true
  [[ -n "${DNS_PID:-}" ]] && kill "$DNS_PID" 2>/dev/null || true
  for p in "${TLS_PID:-}" "${KAFKA_PID:-}" "${VMHTTP_PID:-}"; do [[ -n "$p" ]] && kill "$p" 2>/dev/null || true; done
  pkill -f "http.server 18090" 2>/dev/null || true
  ip netns del "$NS" 2>/dev/null || true
  ip link del "$HOST_IF" 2>/dev/null || true
  [[ -d "$CG" ]] && rmdir "$CG" 2>/dev/null || true
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
    c = s.recv(1 << 20)
    if not c:
        break
    buf += c
print(buf.decode().strip())
PY
}

must() {
  local out
  out=$(cat)
  if ! python3 -c 'import json,sys; sys.exit(0 if json.load(sys.stdin).get("ok") else 1)' <<<"$out"; then
    echo "WARN  request failed: $out"
  fi
}

# jpath OP EXPR: evaluate a Python expression over the response data `d`.
jpath() {
  req "{\"op\":\"$1\"}" | python3 -c "import json,sys; r=json.load(sys.stdin); d=r.get('data', r); print($2)"
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

edge() { req "{\"op\":\"vm_edge_sync\",\"state\":$1}" | must; }
enforce() { req '{"op":"set_mode","mode":"enforce","lease_secs":60}' | must; }
observe() { req '{"op":"set_mode","mode":"observe"}' | must; }
tap_stat() { jpath vm_edge_status "sum(t['stats']['$1'] for t in d['taps'])"; }
gt() { [[ "$1" -gt "$2" ]]; }
not() { ! "$@"; }
auth_has() { req '{"op":"vm_auth_table"}' | grep -q "$1"; }

ping_ok() { ip netns exec "$NS" ping -c1 -W1 "$HOST_IP" >/dev/null 2>&1; }
ping_blocked() { ! ping_ok; }
http_ok() { ip netns exec "$NS" curl -s -m3 -o /dev/null "http://$HOST_IP:18080/"; }

ip netns add "$NS"
ip link add "$HOST_IF" type veth peer name "$PEER_IF"
ip link set "$PEER_IF" netns "$NS"
ip addr add "$HOST_IP/30" dev "$HOST_IF"
ip link set "$HOST_IF" up
ip netns exec "$NS" ip addr add "$VM_IP/30" dev "$PEER_IF"
ip netns exec "$NS" ip link set "$PEER_IF" up
ip netns exec "$NS" ip link set lo up

head -c 393216 /dev/urandom >"$WORK/blob"
(cd "$WORK" && exec python3 -m http.server 18080 --bind "$HOST_IP" >/dev/null 2>&1) &
HTTP_PID=$!

cat >"$WORK/bpfd-state.json" <<EOF
{"telemetry": {"flows": true, "iface_patterns": ["$HOST_IF"]}}
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
sleep 0.5

# ---- VM edge ----------------------------------------------------------------

VMSPEC="{\"name\":\"$VM\",\"addresses\":[\"$VM_IP\"],\"taps\":[\"$HOST_IF\"]"
edge "{\"vms\":[$VMSPEC}]}"
check "edge: tap programmed" [ "$(jpath vm_edge_status "len(d['taps'])")" = 1 ]
check "edge: edge programs chained on tap" bash -c "tc filter show dev $HOST_IF ingress 2>/dev/null | grep -q mn_vm_edge_in || bpftool net show dev $HOST_IF 2>/dev/null | grep -q mn_vm_edge_in"
check "edge: baseline ping" ping_ok
check "edge: baseline http" http_ok
check "edge: counters move" gt "$(tap_stat out_pkts)" 0

edge "{\"vms\":[$VMSPEC,\"isolate_egress\":true}]}"
check "observe: isolation does not block" ping_ok
check "observe: miss counted" gt "$(tap_stat observed)" 0

enforce
check "enforce: isolated VM cannot ping" ping_blocked
http_blocked() { ! http_ok; }
check "enforce: isolated VM cannot reach http" http_blocked
check "enforce: denials counted" gt "$(tap_stat denied)" 0

edge "{\"vms\":[$VMSPEC,\"isolate_egress\":true}],\"policy\":[{\"group\":\"vm:$VM\",\"peer\":\"world\",\"egress\":true,\"proto\":6,\"port\":18080}]}"
check "enforce: allow rule opens tcp/18080" http_ok
check "enforce: ping still blocked" ping_blocked
check "edge: status reports enforcing" [ "$(jpath vm_edge_status "d['enforcing']")" = True ]

observe
check "observe again: ping restored" ping_ok

edge "{\"vms\":[$VMSPEC,\"ingress_mbps\":1}]}"
SPEED=$(ip netns exec "$NS" curl -s -m25 -o /dev/null -w '%{speed_download}' "http://$HOST_IP:18080/blob" || echo 0)
SPEED=${SPEED%.*}
echo "      download with 1 Mbit/s cap: ${SPEED} B/s"
check "rate: ingress cap holds (< 250 kB/s)" bash -c "[ ${SPEED:-0} -gt 0 ] && [ ${SPEED:-0} -lt 250000 ]"
check "rate: drops counted" gt "$(tap_stat rate_dropped)" 0

edge '{"vms":[]}'
check "edge: empty state unprograms tap" [ "$(jpath vm_edge_status "len(d['taps'])")" = 0 ]
check "edge: no edge program left" bash -c "! bpftool net show dev $HOST_IF 2>/dev/null | grep -q mn_vm_edge"
check "edge: unrestricted after unprogram" ping_ok

# ---- VM network policy (compiled identity rules) ------------------------------
# Same shape the netpol compiler emits: per-VM identity, peers by address or
# prefix, identity rules with ranges / deny / ICMP type+1, flow log on.

VMID=5000
CIDR_ID=2147483700
NPVM="{\"name\":\"$VM\",\"addresses\":[\"$VM_IP\"],\"taps\":[\"$HOST_IF\"],\"identity\":$VMID,\"isolate_egress\":true"
HOSTPEER="{\"cidr\":\"$HOST_IP\",\"identity\":1,\"name\":\"host\"}"
R_RANGE="{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":true,\"proto\":6,\"port\":18070,\"port_end\":18090,\"source\":\"smoke spec.egress[0]\"}"
R_ICMP="{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":true,\"proto\":1,\"port\":9,\"source\":\"smoke spec.egress[1]\"}"
R_DENY="{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":true,\"proto\":6,\"port\":18080,\"deny\":true,\"source\":\"smoke spec.egressDeny[0]\"}"
R_CIDR="{\"subject_identity\":$VMID,\"peer_identity\":$CIDR_ID,\"egress\":true,\"proto\":6,\"port\":18080,\"source\":\"smoke spec.egress[2]\"}"
np() { edge "{\"vms\":[$1}],\"peers\":[$2],\"policy\":[$3],\"flow_log\":true,\"owner\":\"smoke\"}"; }
flow_has() {
  sleep 0.5
  req '{"op":"vm_flows","limit":1000}' | python3 -c "import json,sys; d=json.load(sys.stdin).get('data') or []; sys.exit(0 if any($1 for f in d) else 1)"
}
host_ping_vm() { ping -c1 -W1 "$VM_IP" >/dev/null 2>&1; }

np "$NPVM" "$HOSTPEER" "$R_RANGE"
check "netpol: identity + peers reported" [ "$(jpath vm_edge_status "(d['owner'], d['peers'], d['flow_log'])")" = "('smoke', 1, True)" ]
check "netpol observe: default-deny miss still forwards" ping_ok
check "netpol observe: AUDIT flow for the miss" flow_has "f['verdict']=='AUDIT' and f['proto']=='icmp' and f['drop_reason']=='default-deny'"

enforce
check "netpol enforce: port range allows tcp/18080" http_ok
check "netpol enforce: FORWARDED flow attributed to rule" flow_has "f['verdict']=='FORWARDED' and f['dst_port']==18080 and f.get('policy')=='smoke spec.egress[0]'"
check "netpol enforce: ICMP not allowed yet" ping_blocked
check "netpol enforce: DROPPED flow for ICMP" flow_has "f['verdict']=='DROPPED' and f['proto']=='icmp' and f.get('icmp_type')==8"

np "$NPVM" "$HOSTPEER" "$R_RANGE,$R_ICMP"
check "netpol enforce: ICMP echo-request rule allows ping" ping_ok

np "$NPVM" "$HOSTPEER" "$R_RANGE,$R_ICMP,$R_DENY"
check "netpol enforce: deny beats the allow range" bash -c "! ip netns exec $NS curl -s -m3 -o /dev/null http://$HOST_IP:18080/"
check "netpol enforce: policy-deny flow" flow_has "f['verdict']=='DROPPED' and f.get('drop_reason')=='policy-deny' and f.get('policy')=='smoke spec.egressDeny[0]'"

np "$NPVM" "{\"cidr\":\"10.199.81.0/30\",\"identity\":$CIDR_ID,\"name\":\"10.199.81.0/30\"}" "$R_CIDR"
check "netpol enforce: CIDR peer via LPM allows tcp/18080" http_ok
check "netpol enforce: host identity rules no longer match (ping blocked)" ping_blocked

NPVM_OPEN="{\"name\":\"$VM\",\"addresses\":[\"$VM_IP\"],\"taps\":[\"$HOST_IF\"],\"identity\":$VMID"
np "$NPVM_OPEN" "$HOSTPEER" "{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":false,\"proto\":1,\"port\":0,\"deny\":true,\"source\":\"smoke spec.ingressDeny[0]\"}"
check "netpol enforce: ingressDeny applies without isolation" bash -c "! ping -c1 -W1 $VM_IP >/dev/null 2>&1"
check "netpol enforce: VM egress unaffected by ingress deny" http_ok

# toFQDNs: a stub resolver on the host answers svc.smoke.test → HOST_IP. No
# host peer, so HOST_IP starts as `world` and is learned as its own identity.
python3 - "$HOST_IP" >"$WORK/dns.log" 2>&1 <<'PY' &
import socket, struct, sys
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.bind((sys.argv[1], 53))
ip = socket.inet_aton(sys.argv[1])
while True:
    q, a = s.recvfrom(512)
    i, name = 12, []
    while q[i]:
        name.append(q[i + 1:i + 1 + q[i]].decode()); i += q[i] + 1
    end = i + 5
    ok = ".".join(name).lower() == "svc.smoke.test"
    hdr = q[:2] + struct.pack(">HHHHH", 0x8180 if ok else 0x8183, 1, int(ok), 0, 0)
    ans = (b"\xc0\x0c" + struct.pack(">HHIH", 1, 1, 5, 4) + ip) if ok else b""
    s.sendto(hdr + q[12:end] + ans, a)
PY
DNS_PID=$!
resolve() {
  ip netns exec "$NS" python3 - "$HOST_IP" "$1" <<'PY'
import socket, struct, sys
q = struct.pack(">HHHHHH", 0x4d4e, 0x0100, 1, 0, 0, 0)
q += b"".join(bytes([len(p)]) + p.encode() for p in sys.argv[2].split(".")) + b"\x00" + struct.pack(">HH", 1, 1)
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.settimeout(2)
s.sendto(q, (sys.argv[1], 53)); r = s.recv(512)
sys.exit(0 if struct.unpack(">H", r[6:8])[0] > 0 else 1)
PY
}
fqdn_has() { req '{"op":"vm_fqdn_cache"}' | python3 -c "import json,sys; d=json.load(sys.stdin).get('data') or []; sys.exit(0 if any($1 for e in d) else 1)"; }
R_DNS="{\"subject_identity\":$VMID,\"peer_identity\":2,\"egress\":true,\"proto\":17,\"port\":53,\"source\":\"smoke spec.egress[3]\"}"
F_SVC="{\"pattern\":\"svc.smoke.test\",\"subject_identity\":$VMID,\"proto\":6,\"port\":18080,\"source\":\"smoke spec.egress[4]\"}"
npf() { edge "{\"vms\":[$NPVM}],\"policy\":[$1],\"fqdn\":[$2],\"flow_log\":true,\"owner\":\"smoke\"}"; }
sleep 0.3
npf "$R_DNS" "$F_SVC"
check "fqdn: status counts the rule, tap snoops DNS" [ "$(jpath vm_edge_status "(d['fqdn_rules'], 'fqdn' in d['taps'][0]['flags'])")" = "(1, True)" ]
check "fqdn enforce: blocked before the lookup" bash -c "! ip netns exec $NS curl -s -m2 -o /dev/null http://$HOST_IP:18080/"
check "fqdn enforce: DNS to world allowed" resolve svc.smoke.test
sleep 1.5
check "fqdn: binding learned from the reply" fqdn_has "e['name']=='svc.smoke.test' and e['address']=='$HOST_IP' and e['identity']>=2**31 and e['vm']=='$VM'"
check "fqdn enforce: allowed after the lookup" http_ok
check "fqdn enforce: flow attributed to the toFQDNs rule" flow_has "f['verdict']=='FORWARDED' and f['dst_port']==18080 and f.get('policy')=='smoke spec.egress[4]'"
resolve other.smoke.test || true
sleep 1
check "fqdn: unmatched / NXDOMAIN names learn nothing" [ "$(req '{"op":"vm_fqdn_cache"}' | python3 -c "import json,sys; print(len(json.load(sys.stdin).get('data') or []))")" = 1 ]
R_WDENY="{\"subject_identity\":$VMID,\"peer_identity\":2,\"egress\":true,\"proto\":6,\"port\":18080,\"deny\":true,\"source\":\"smoke spec.egressDeny[1]\"}"
npf "$R_DNS,$R_WDENY" "$F_SVC"
check "fqdn enforce: world deny still wins for a learned address" bash -c "! ip netns exec $NS curl -s -m2 -o /dev/null http://$HOST_IP:18080/"
npf "$R_DNS" ""
check "fqdn: removing the rule drops the binding" bash -c "! ip netns exec $NS curl -s -m2 -o /dev/null http://$HOST_IP:18080/"
kill "$DNS_PID" 2>/dev/null || true

# ---- L7 (native, bpfd-parsed) ----------------------------------------------
# Requests on L7 ports are held in the kernel until bpfd decides; denied
# HTTP gets a 403 from bpfd, other protocols a RST, DNS a REFUSED.

echo allowed >"$WORK/allowed.txt"
echo secret >"$WORK/secret.txt"
HOSTPEER_W="{\"cidr\":\"$HOST_IP\",\"identity\":1,\"name\":\"host\"}"
R_L7="{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":true,\"proto\":6,\"port\":18080,\"l7\":true,\"source\":\"smoke spec.egress[5]\"}"
L7_HTTP="{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":true,\"proto\":6,\"port\":18080,\"rules\":{\"http\":[{\"method\":\"GET\",\"path\":\"/allowed.*\"}]},\"source\":\"smoke spec.egress[5]\"}"
npl() { edge "{\"vms\":[$NPVM}],\"peers\":[$HOSTPEER_W],\"policy\":[$1],\"l7\":[$2],\"flow_log\":true,\"owner\":\"smoke\"}"; }
code() { ip netns exec "$NS" curl -s -m6 -o /dev/null -w '%{http_code}' "$@" || true; }
enforce
npl "$R_L7" "$L7_HTTP"
check "l7: status counts the rule, tap flagged" [ "$(jpath vm_edge_status "(d['l7_rules'], 'l7_auth' in d['taps'][0]['flags'])")" = "(1, True)" ]
check "l7 enforce: GET /allowed.txt passes" [ "$(code "http://$HOST_IP:18080/allowed.txt")" = 200 ]
check "l7 enforce: GET /secret.txt gets 403 from bpfd" [ "$(code "http://$HOST_IP:18080/secret.txt")" = 403 ]
check "l7 enforce: POST is denied" [ "$(code -X POST -d x "http://$HOST_IP:18080/allowed.txt")" = 403 ]
check "l7 enforce: keep-alive second request checked too" bash -c "ip netns exec $NS curl -s -m8 -o /dev/null -o /dev/null -w '%{http_code} ' http://$HOST_IP:18080/allowed.txt http://$HOST_IP:18080/secret.txt | grep -q '^200 403'"
check "l7: DROPPED flow with the request" flow_has "f['verdict']=='DROPPED' and f.get('l7_type')=='http' and 'GET' in (f.get('l7') or '') and '/secret.txt' in f['l7'] and f.get('drop_reason')=='l7-deny'"
check "l7: FORWARDED flow for the allowed request" flow_has "f['verdict']=='FORWARDED' and f.get('l7_type')=='http' and '/allowed.txt' in (f.get('l7') or '')"
observe
check "l7 observe: denied request still served" [ "$(code "http://$HOST_IP:18080/secret.txt")" = 200 ]
check "l7 observe: AUDIT flow" flow_has "f['verdict']=='AUDIT' and f.get('l7_type')=='http'"

# TLS serverNames: SNI of the ClientHello.
openssl req -x509 -newkey rsa:2048 -nodes -subj /CN=smoke -keyout "$WORK/k.pem" -out "$WORK/c.pem" -days 1 >/dev/null 2>&1
openssl s_server -quiet -accept "$HOST_IP:18443" -www -cert "$WORK/c.pem" -key "$WORK/k.pem" >/dev/null 2>&1 &
TLS_PID=$!
R_TLS="{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":true,\"proto\":6,\"port\":18443,\"l7\":true,\"source\":\"smoke spec.egress[6]\"}"
L7_TLS="{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":true,\"proto\":6,\"port\":18443,\"rules\":{\"server_names\":[\"ok.smoke.test\"]},\"source\":\"smoke spec.egress[6]\"}"
tls() { ip netns exec "$NS" curl -sk -m6 -o /dev/null --resolve "$1:18443:$HOST_IP" "https://$1:18443/"; }
sleep 0.5
enforce
npl "$R_TLS" "$L7_TLS"
check "l7 tls: allowed SNI connects" tls ok.smoke.test
check "l7 tls: other SNI reset" bash -c "! ip netns exec $NS curl -sk -m6 -o /dev/null --resolve bad.smoke.test:18443:$HOST_IP https://bad.smoke.test:18443/"
check "l7 tls: flow names the SNI" flow_has "f.get('l7_type')=='tls' and 'bad.smoke.test' in (f.get('l7') or '') and f['verdict']=='DROPPED'"
kill "$TLS_PID" 2>/dev/null || true

# Kafka: a stub broker answers any request; produce to `orders` only.
python3 - "$HOST_IP" >/dev/null 2>&1 <<'PY' &
import socket, struct, sys, threading
s = socket.socket(); s.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
s.bind((sys.argv[1], 19092)); s.listen(8)
def serve(c):
    try:
        while True:
            h = c.recv(4)
            if len(h) < 4: return
            n = struct.unpack(">i", h)[0]; b = b""
            while len(b) < n:
                x = c.recv(n - len(b))
                if not x: return
                b += x
            c.sendall(struct.pack(">ii", 4, struct.unpack(">i", b[4:8])[0]))
    except OSError:
        pass
while True:
    c, _ = s.accept(); threading.Thread(target=serve, args=(c,), daemon=True).start()
PY
KAFKA_PID=$!
produce() {
  ip netns exec "$NS" python3 - "$HOST_IP" "$1" <<'PY'
import socket, struct, sys
def s16(x): return struct.pack(">h", len(x)) + x
topic = sys.argv[2].encode()
body = struct.pack(">hi", 1, 1000) + struct.pack(">i", 1) + s16(topic) + struct.pack(">i", 1) + struct.pack(">i", 0) + struct.pack(">i", 0)
req = struct.pack(">hhi", 0, 0, 7) + s16(b"smoke") + body
try:
    c = socket.create_connection((sys.argv[1], 19092), timeout=6)
    c.sendall(struct.pack(">i", len(req)) + req)
    r = c.recv(8)
except OSError:
    sys.exit(1)
sys.exit(0 if len(r) == 8 and struct.unpack(">ii", r)[1] == 7 else 1)
PY
}
R_KAFKA="{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":true,\"proto\":6,\"port\":19092,\"l7\":true,\"source\":\"smoke spec.egress[7]\"}"
L7_KAFKA="{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":true,\"proto\":6,\"port\":19092,\"rules\":{\"kafka\":[{\"role\":\"produce\",\"topic\":\"orders\"}]},\"source\":\"smoke spec.egress[7]\"}"
sleep 0.5
enforce
npl "$R_KAFKA" "$L7_KAFKA"
check "l7 kafka: produce to orders answered" produce orders
check "l7 kafka: produce to payments reset" not produce payments
check "l7 kafka: flow names the topic" flow_has "f.get('l7_type')=='kafka' and 'payments' in (f.get('l7') or '') and f['verdict']=='DROPPED'"
kill "$KAFKA_PID" 2>/dev/null || true

# DNS rules: bpfd forwards allowed queries and answers REFUSED otherwise.
python3 - "$HOST_IP" >"$WORK/dns2.log" 2>&1 <<'PY' &
import socket, struct, sys
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
s.bind((sys.argv[1], 53))
ip = socket.inet_aton(sys.argv[1])
while True:
    q, a = s.recvfrom(512)
    i = 12
    while q[i]: i += q[i] + 1
    end = i + 5
    hdr = q[:2] + struct.pack(">HHHHH", 0x8180, 1, 1, 0, 0)
    s.sendto(hdr + q[12:end] + b"\xc0\x0c" + struct.pack(">HHIH", 1, 1, 5, 4) + ip, a)
PY
DNS_PID=$!
rcode() {
  ip netns exec "$NS" python3 - "$HOST_IP" "$1" <<'PY'
import socket, struct, sys
q = struct.pack(">HHHHHH", 0x4d4f, 0x0100, 1, 0, 0, 0)
q += b"".join(bytes([len(p)]) + p.encode() for p in sys.argv[2].split(".")) + b"\x00" + struct.pack(">HH", 1, 1)
s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.settimeout(3)
s.sendto(q, (sys.argv[1], 53))
try:
    r = s.recv(512); print(r[3] & 15, struct.unpack(">H", r[6:8])[0])
except socket.timeout:
    print("timeout")
PY
}
R_DNSL7="{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":true,\"proto\":17,\"port\":53,\"l7\":true,\"source\":\"smoke spec.egress[8]\"}"
L7_DNS="{\"subject_identity\":$VMID,\"peer_identity\":1,\"egress\":true,\"proto\":17,\"port\":53,\"rules\":{\"dns\":[\"*.smoke.test\"]},\"source\":\"smoke spec.egress[8]\"}"
sleep 0.3
enforce
npl "$R_DNSL7" "$L7_DNS"
check "l7 dns: allowed name forwarded by bpfd and answered" [ "$(rcode svc.smoke.test)" = "0 1" ]
check "l7 dns: other name REFUSED" [ "$(rcode example.org)" = "5 0" ]
check "l7 dns: flow names the query" flow_has "f.get('l7_type')=='dns' and 'example.org' in (f.get('l7') or '') and f['verdict']=='DROPPED'"
kill "$DNS_PID" 2>/dev/null || true

# ---- authentication + source guard -------------------------------------------
# HOST_IP is a fleet VM peer (identity 5001) for `required`; the host
# connects to a server inside the netns.

SPOOF_IP=10.199.83.3
(ip netns exec "$NS" python3 -m http.server 18090 --bind "$VM_IP" >/dev/null 2>&1) &
VMHTTP_PID=$!
AVM="{\"name\":\"$VM\",\"addresses\":[\"$VM_IP\"],\"taps\":[\"$HOST_IF\"],\"identity\":$VMID,\"isolate_ingress\":true"
FLEETPEER="{\"cidr\":\"$HOST_IP\",\"identity\":5001,\"name\":\"fleet-peer\"},{\"cidr\":\"$SPOOF_IP\",\"identity\":5002,\"name\":\"other-vm\"}"
R_AUTH="{\"subject_identity\":$VMID,\"peer_identity\":5001,\"egress\":false,\"proto\":6,\"port\":18090,\"auth\":1,\"source\":\"smoke spec.ingress[0]\"}"
R_AFAIL="{\"subject_identity\":$VMID,\"peer_identity\":5001,\"egress\":false,\"proto\":6,\"port\":18090,\"auth\":2,\"source\":\"smoke spec.ingress[0]\"}"
npa() { edge "{\"vms\":[$AVM}],\"peers\":[$FLEETPEER],\"policy\":[$1],\"flow_log\":true,\"owner\":\"smoke\"}"; }
to_vm() { curl -s -m6 -o /dev/null "http://$VM_IP:18090/"; }
sleep 0.5
enforce
npa "$R_AFAIL"
check "auth: source guard on the tap" [ "$(jpath vm_edge_status "'source_guard' in d['taps'][0]['flags']")" = True ]
check "auth enforce: test-always-fail blocks" bash -c "! curl -s -m3 -o /dev/null http://$VM_IP:18090/"
check "auth: flow says auth-test-always-fail" flow_has "f.get('drop_reason')=='auth-test-always-fail' and f['verdict']=='DROPPED'"
npa "$R_AUTH"
check "auth enforce: required authenticates the fleet VM peer" to_vm
check "auth: first SYN dropped as auth-required" flow_has "f.get('drop_reason')=='auth-required'"
check "auth: table shows the pair" auth_has 'authenticated (fleet VM fleet-peer)'
check "auth: status counts it" [ "$(jpath vm_edge_status "d['auth_entries']")" = 1 ]
ip netns exec "$NS" ip addr add "$SPOOF_IP/32" dev "$PEER_IF"
ip route add "$SPOOF_IP/32" dev "$HOST_IF"
from_spoof() {
  ip netns exec "$NS" python3 -c "import socket,sys; s=socket.socket(); s.settimeout(3); s.bind(('$SPOOF_IP',0)); s.connect(('$HOST_IP',18080))" 2>/dev/null
}
check "guard: own address still works" http_ok
check "guard enforce: spoofed source (another identity) dropped" not from_spoof
check "guard: flow says spoofed-source" flow_has "f.get('drop_reason')=='spoofed-source' and f['src']=='$SPOOF_IP'"
observe
check "guard observe: spoofed source only audited" from_spoof
ip netns exec "$NS" ip addr del "$SPOOF_IP/32" dev "$PEER_IF"
kill "$VMHTTP_PID" 2>/dev/null || true

observe
check "netpol observe: ingressDeny only audited" host_ping_vm

edge '{"vms":[]}'
check "netpol: cleared" [ "$(jpath vm_edge_status "len(d['taps'])")" = 0 ]

# ---- QEMU sandbox -----------------------------------------------------------

mkdir -p "$CG"
# /dev/kmsg (c 1:11) is outside the default QEMU device list.
# in_cg CMD...: run CMD inside the fake QEMU scope.
in_cg() { bash -c "echo \$\$ > $CG/cgroup.procs && exec \"\$@\"" _ "$@"; }
dev_ok() { in_cg python3 -c "open('/dev/kmsg','rb').close()" 2>/dev/null; }
cg_http() { in_cg curl -s -m2 -o /dev/null "http://$HOST_IP:$1/"; }
cg_dropped() {
  local rc=0
  in_cg curl -s -m2 -o /dev/null "http://$HOST_IP:$1/" || rc=$?
  [[ $rc -eq 28 ]]
}

req "{\"op\":\"vm_sandbox_configure\",\"config\":{\"mode\":\"enforce\",\"egress_ports\":[\"18080\"]}}" | must
req "{\"op\":\"vm_sandbox_attach\",\"vm\":\"$VM\",\"cgroup\":\"$CG_REL\"}" | must
check "sandbox: attached to fake scope" [ "$(jpath vm_sandbox_status "d['attached'].get('$VM','')")" = "$CG_REL" ]
check "sandbox: device + egress programs on cgroup" bash -c "bpftool cgroup show $CG | grep -q mn_qemu_device && bpftool cgroup show $CG | grep -q mn_qemu_egress"
check "sandbox: allowed device works (/dev/null)" in_cg python3 -c "open('/dev/null','rb').close()"
check "sandbox no lease: unlisted device still opens" dev_ok
check "sandbox no lease: allowed port reachable" cg_http 18080
cg_http 18081 || true
hits() { jpath vm_sandbox_status "[h['target'] for h in d['$1']]" | grep -q "$2"; }
check "sandbox: device hit recorded" hits device_hits '1:11'
check "sandbox: egress hit recorded" hits egress_hits '18081'

enforce
check "sandbox enforce: status enforcing" [ "$(jpath vm_sandbox_status "d['enforcing']")" = True ]
dev_denied() { ! dev_ok; }
check "sandbox enforce: unlisted device denied" dev_denied
check "sandbox enforce: allowed device works" in_cg python3 -c "open('/dev/null','rb').close()"
check "sandbox enforce: allowed port reachable" cg_http 18080
check "sandbox enforce: other port dropped (timeout, not refused)" cg_dropped 18081
check "sandbox enforce: host outside scope unaffected" python3 -c "open('/dev/kmsg','rb').close()"

observe
check "sandbox observe: device opens again" dev_ok

req "{\"op\":\"vm_sandbox_detach\",\"vm\":\"$VM\"}" | must
check "sandbox: detached" bash -c "! bpftool cgroup show $CG 2>/dev/null | grep -q mn_qemu"
rmdir "$CG" 2>/dev/null || true

if grep -qiE "verifier|panicked" "$WORK/bpfd.log"; then
  echo "---- bpfd log (verifier/panic) ----"
  grep -iE -A20 "verifier|panicked" "$WORK/bpfd.log" | head -60
fi
echo "passed=$PASS failed=$FAIL"
[[ $FAIL -eq 0 ]]

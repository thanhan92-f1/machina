#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0
#
# VM network policy against two real libvirt VMs (Debian cloud image on the
# `default` NAT network): observe first, then a short enforcement lease,
# then the TLS-intercepting proxy (terminatingTLS + header rewrites,
# originatingTLS). Creates np-client / np-server and deletes them, the
# policies, the proxy secret and the base image on exit; the edge is always
# returned to observe.
#
# Only run on a disposable test host: while the lease is held the edge
# enforces on every tap that has policy state.
#
#   printf '%s\n' "$PASS" | sudo -n true && bash scripts/bpf/vm-netpol-realvm.sh
#
# The machina password is read from stdin (MACHINA_USER defaults to $USER).
set -uo pipefail

read -r MACHINA_PASS
export MACHINA_USER="${MACHINA_USER:-$USER}" MACHINA_PASS MACHINA_URL="${MACHINA_URL:-https://127.0.0.1:5092}" NO_COLOR=1
HERE="$(cd "$(dirname "$0")/../.." && pwd)"
M="$HERE/machinactl"
LEASE="${LEASE:-300}"
SECRET_DIR=/etc/machina/netpol-secrets/default/np-intercept
IMG_URL="${IMG_URL:-https://cloud.debian.org/images/cloud/bookworm/latest/debian-12-genericcloud-amd64.qcow2}"
POOL=/var/lib/libvirt/images
BASE="$POOL/np-realvm-base.qcow2"
W="$(mktemp -d /tmp/np-realvm.XXXXXX)"
VMS=(np-client np-server)
P=0; F=0
exec 3>&2

ok() { echo "PASS  $1"; P=$((P+1)); }
bad() { echo "FAIL  $1"; F=$((F+1)); }
check() { local n=$1; shift; if "$@" >/dev/null 2>&1; then ok "$n"; else bad "$n"; fi; }

bpfd() {
    sudo -n python3 - "$1" <<'PY'
import json, socket, sys
s = socket.socket(socket.AF_UNIX); s.connect('/run/machina-bpf/bpfd.sock')
s.sendall(sys.argv[1].encode() + b'\n'); b = b''
while not b.endswith(b'\n'):
    c = s.recv(1 << 20)
    if not c: break
    b += c
r = json.loads(b)
print(json.dumps(r.get('data', r)))
sys.exit(0 if r.get('ok', True) is not False else 1)
PY
}
observe() { bpfd '{"op":"set_mode","mode":"observe"}' >/dev/null; }

cleanup() {
    observe
    for v in "${VMS[@]}"; do "$M" vm release "$v" >/dev/null 2>&1; done
    "$M" netpol delete np-realvm >/dev/null 2>&1
    "$M" netpol delete np-realvm-proxy >/dev/null 2>&1
    sudo -n rm -rf "$SECRET_DIR"
    for v in "${VMS[@]}"; do
        sudo -n virsh destroy "$v" >/dev/null 2>&1
        sudo -n virsh undefine "$v" --nvram >/dev/null 2>&1 || sudo -n virsh undefine "$v" >/dev/null 2>&1
        sudo -n rm -f "$POOL/$v.qcow2" "$POOL/$v-seed.iso"
        "$M" vm label "$v" app- >/dev/null 2>&1
    done
    sudo -n rm -f "$BASE"
    rm -rf "$W"
    echo "cleanup: VMs, policy and image removed; edge mode $(bpfd '{"op":"vm_edge_status"}' | python3 -c 'import json,sys; print("enforce" if json.load(sys.stdin).get("enforcing") else "observe")')"
}
trap cleanup EXIT

echo "== VMs =="
for v in "${VMS[@]}"; do
    sudo -n virsh dominfo "$v" >/dev/null 2>&1 && { echo "$v already exists; refusing to touch it" >&2; trap - EXIT; exit 1; }
done
ssh-keygen -q -t ed25519 -N '' -f "$W/key"
sudo -n curl -fsSL -o "$BASE" "$IMG_URL" || { bad "download $IMG_URL"; exit 1; }
for v in "${VMS[@]}"; do
    cat > "$W/$v-user" <<EOF
#cloud-config
hostname: $v
users:
  - name: np
    sudo: ALL=(ALL) NOPASSWD:ALL
    shell: /bin/bash
    ssh_authorized_keys: ["$(cat "$W/key.pub")"]
write_files:
  - path: /srv/echo.py
    content: |
      import http.server, ssl, sys
      class H(http.server.BaseHTTPRequestHandler):
          def do_GET(self):
              b = (self.requestline + "\n" + str(self.headers)).encode()
              self.send_response(200)
              self.send_header("Content-Length", str(len(b)))
              self.end_headers()
              self.wfile.write(b)
          do_POST = do_GET
      s = http.server.ThreadingHTTPServer(("", int(sys.argv[1])), H)
      if len(sys.argv) > 2:
          c = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
          c.load_cert_chain(sys.argv[2], sys.argv[3])
          s.socket = c.wrap_socket(s.socket, server_side=True)
      s.serve_forever()
runcmd:
  - [mkdir, -p, /srv]
  - [sh, -c, "echo ok > /srv/ok; echo secret > /srv/secret"]
  - [systemd-run, --unit, np80, python3, -m, http.server, "80", --directory, /srv]
  - [systemd-run, --unit, np8080, python3, -m, http.server, "8080", --directory, /srv]
  - [systemd-run, --unit, np443, python3, /srv/echo.py, "443"]
EOF
    printf 'instance-id: %s-%s\nlocal-hostname: %s\n' "$v" "$$" "$v" > "$W/$v-meta"
    sudo -n cloud-localds "$POOL/$v-seed.iso" "$W/$v-user" "$W/$v-meta"
    sudo -n qemu-img create -q -f qcow2 -F qcow2 -b "$BASE" "$POOL/$v.qcow2" 8G
    sudo -n virt-install --name "$v" --memory 1024 --vcpus 1 --import --osinfo detect=on,require=off \
        --disk "path=$POOL/$v.qcow2,format=qcow2,bus=virtio" --disk "path=$POOL/$v-seed.iso,device=cdrom" \
        --network network=default,model=virtio --graphics none --noautoconsole >/dev/null \
        && ok "virt-install $v" || bad "virt-install $v"
done

ip_of() { sudo -n virsh domifaddr "$1" --source lease 2>/dev/null | awk '/ipv4/ {sub(/\/.*/, "", $4); print $4; exit}'; }
for _ in $(seq 90); do
    CIP=$(ip_of np-client); SIP=$(ip_of np-server)
    [[ -n "$CIP" && -n "$SIP" ]] && break
    sleep 2
done
[[ -n "${CIP:-}" && -n "${SIP:-}" ]] && ok "DHCP leases: client $CIP server $SIP" || { bad "DHCP leases"; exit 1; }
vssh() { local h=$1; shift; ssh -i "$W/key" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=5 -o LogLevel=ERROR "np@$h" "$@"; }
cssh() { vssh "$CIP" "$@"; }
sssh() { vssh "$SIP" "$@"; }
for _ in $(seq 90); do cssh true 2>/dev/null && break; sleep 2; done
check "ssh into client" cssh true
for _ in $(seq 60); do curl -fs -m 2 "http://$SIP/ok" >/dev/null && curl -fs -m 2 "http://$SIP:8080/ok" >/dev/null && break; sleep 2; done
check "server answers on 80 and 8080" curl -fs -m 2 "http://$SIP:8080/ok"

# Test CA + server certificate for the proxy phase (shipped before any lease:
# the server's ingress policy keeps the host's ssh out while enforcing).
openssl req -x509 -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -days 1 -subj /CN=np-test-ca \
    -addext basicConstraints=critical,CA:TRUE -keyout "$W/ca.key" -out "$W/ca.crt" 2>/dev/null
openssl req -newkey ec -pkeyopt ec_paramgen_curve:P-256 -nodes -subj /CN=np-server.test \
    -keyout "$W/tls.key" -out "$W/tls.csr" 2>/dev/null
printf 'subjectAltName=DNS:np-server.test,IP:%s\n' "$SIP" > "$W/san"
openssl x509 -req -in "$W/tls.csr" -CA "$W/ca.crt" -CAkey "$W/ca.key" -CAcreateserial -days 1 \
    -extfile "$W/san" -out "$W/tls.crt" 2>/dev/null
for _ in $(seq 60); do sssh true 2>/dev/null && break; sleep 2; done
cssh "cat > /tmp/ca.crt" < "$W/ca.crt"
sssh "cat > /tmp/tls.crt" < "$W/tls.crt"
sssh "cat > /tmp/tls.key" < "$W/tls.key"
sssh "sudo systemd-run --unit np8443 python3 /srv/echo.py 8443 /tmp/tls.crt /tmp/tls.key" >/dev/null 2>&1
tls_echo() { curl -fs -m 3 --cacert "$W/ca.crt" "https://$SIP:8443/hdr" | grep -q "GET /hdr"; }
for _ in $(seq 10); do tls_echo && break; sleep 1; done
check "server answers TLS on 8443" tls_echo

# HTTP status of a request from the client to the server ("000" = no answer).
get() { local path=$1; shift; cssh "curl -s -m 4 -o /dev/null -w '%{http_code}' $* http://$SIP$path" 2>/dev/null; }
is() { local want=$1; shift; [[ "$(get "$@")" == "$want" ]]; }
taps_programmed() {
    bpfd '{"op":"vm_edge_status"}' | python3 -c 'import json,sys; sys.exit(0 if len(json.load(sys.stdin).get("taps", [])) >= 2 else 1)'
}
flow_has() {
    local pat=$1; shift
    local vm=${FLOW_VM:-np-server}
    for _ in 1 2 3 4 5; do
        "$M" flow observe --vm "$vm" --last 300 "$@" > "$W/flows" 2>&1
        grep -q -- "$pat" "$W/flows" && return
        sleep 2
    done
    { echo "      flow observe --vm $vm $*:"; tail -n 8 "$W/flows"; echo "      unfiltered:"; "$M" flow observe --last 8 2>&1; } | sed 's/^/      /' >&3
    return 1
}

check "label client" "$M" vm label np-client app=np-client
check "label server" "$M" vm label np-server app=np-server
cat > "$W/policy.yaml" <<'Y'
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: np-realvm
spec:
  description: real-VM test - client may only GET /ok on the server's port 80
  endpointSelector:
    matchLabels: {app: np-server}
  ingress:
    - fromEndpoints:
        - matchLabels: {app: np-client}
      toPorts:
        - ports: [{port: "80", protocol: TCP}]
          rules:
            http: [{method: GET, path: /ok}]
Y
check "apply policy" "$M" netpol apply -f "$W/policy.yaml"
for _ in $(seq 30); do
    "$M" netpol endpoints 2>/dev/null | grep -q "np-server.*$SIP" && break
    sleep 2; "$M" netpol apply -f "$W/policy.yaml" >/dev/null 2>&1
done
check "server endpoint has its address" bash -c "'$M' netpol endpoints | grep -q 'np-server.*$SIP'"
check "edge programmed the VM taps" taps_programmed
# Learn and replay below judge this run's traffic, not earlier runs'.
check "flow history cleared" bpfd '{"op":"vm_flow_edges_reset"}'

echo "== observe =="
observe
check "observe: GET /ok 200" is 200 /ok
check "observe: GET /secret still 200 (audited)" is 200 /secret
check "observe: port 8080 still open" is 200 :8080/ok
sleep 2
check "observe: AUDIT flow for 8080" flow_has 8080 --verdict AUDIT --port 8080

echo "== enforce (lease ${LEASE}s) =="
check "take enforcement lease" bpfd "{\"op\":\"set_mode\",\"mode\":\"enforce\",\"lease_secs\":$LEASE}"
check "enforce: GET /ok 200" is 200 /ok
check "enforce: GET /secret 403 from L7 rule" is 403 /secret
check "enforce: POST /ok 403" is 403 /ok -X POST
check "enforce: port 8080 dropped" is 000 :8080/ok
check "enforce: host (not np-client) cannot reach server :80" bash -c "! curl -fs -m 3 -o /dev/null http://$SIP/ok"
check "enforce: client without policy keeps egress" cssh "ping -c1 -W2 192.168.122.1"
sleep 2
check "enforce: DROPPED flow for 8080" flow_has 8080 --verdict DROPPED --port 8080
check "enforce: L7 flow records the request" flow_has /secret --port 80

echo "== just-in-time access =="
check "jit: grant np-client → np-server :8080 for 8s" "$M" netpol jit grant --from np-client --to np-server --port 8080 --for 8s --reason realvm
check "jit: 8080 open during the grant" is 200 :8080/ok
check "jit: still only for np-client (host dropped on 8080)" bash -c "! curl -fs -m 3 -o /dev/null http://$SIP:8080/ok"
check "jit: listed" bash -c "'$M' netpol jit | grep -q 'np-client.*np-server.*8080/tcp'"
sleep 10
check "jit: 8080 dropped again after expiry" is 000 :8080/ok
check "jit: grant removed" bash -c "! '$M' netpol get | grep -q jit-np-client"

echo "== proxy: TLS interception + header rewrites =="
check "proxy secret installed" bash -c "sudo -n mkdir -p '$SECRET_DIR' && sudo -n install -m600 '$W/tls.crt' '$W/tls.key' '$W/ca.crt' '$SECRET_DIR/'"
cat > "$W/proxy.yaml" <<'Y'
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: np-realvm-proxy
specs:
  - description: client HTTPS to the server is intercepted and rewritten
    endpointSelector:
      matchLabels: {app: np-client}
    egress:
      - toEndpoints:
          - matchLabels: {app: np-server}
        toPorts:
          - ports: [{port: "443", protocol: TCP}]
            terminatingTLS:
              secret: {name: np-intercept}
            rules:
              http:
                - path: /hdr
                  headerMatches:
                    - {name: X-Team, value: blue, mismatch: REPLACE}
                    - {name: X-Debug, value: "0", mismatch: DELETE}
                    - {name: X-Via, value: machina, mismatch: ADD}
          - ports: [{port: "8443", protocol: TCP}]
            originatingTLS:
              secret: {name: np-intercept}
              trustedCA: ca.crt
            rules:
              http: [{path: /hdr}]
  - description: the server admits the client (not the host) on 443 / 8443
    endpointSelector:
      matchLabels: {app: np-server}
    ingress:
      - fromEndpoints:
          - matchLabels: {app: np-client}
        toPorts:
          - ports: [{port: "443", protocol: TCP}, {port: "8443", protocol: TCP}]
Y
check "apply proxy policy" "$M" netpol apply -f "$W/proxy.yaml"
proxy_up() {
    bpfd '{"op":"vm_edge_status"}' | python3 -c 'import json,sys; sys.exit(0 if json.load(sys.stdin).get("proxy","").startswith("listening") else 1)'
}
for _ in $(seq 10); do proxy_up && break; sleep 1; done
check "bpfd proxy listening" proxy_up
https() { local path=$1; shift; cssh "curl -s -m 6 --cacert /tmp/ca.crt --resolve np-server.test:443:$SIP $* https://np-server.test$path" 2>/dev/null; }
https /hdr -H "'X-Team: red'" -H "'X-Debug: 1'" > "$W/hdr.out"
check "intercepted HTTPS reaches the server" grep -q "GET /hdr" "$W/hdr.out"
check "REPLACE: X-Team rewritten to blue" grep -q "X-Team: blue" "$W/hdr.out"
check "REPLACE: original X-Team gone" bash -c "! grep -q 'X-Team: red' '$W/hdr.out'"
check "DELETE: X-Debug removed" bash -c "! grep -qi 'X-Debug' '$W/hdr.out'"
check "ADD: X-Via added" grep -q "X-Via: machina" "$W/hdr.out"
denied_other() { [[ "$(https /other -o /dev/null -w "'%{http_code}'")" == 403 ]]; }
check "intercepted path outside the rule: 403" denied_other
policy_cert() { cssh "curl -sv -m 6 -o /dev/null --cacert /tmp/ca.crt --resolve np-server.test:443:$SIP https://np-server.test/hdr" 2>&1 | grep -q "issuer: CN=np-test-ca"; }
check "client sees the policy certificate" policy_cert
check "server sees the client identity (host itself is refused on 443)" bash -c "! curl -s -m 3 -o /dev/null http://$SIP:443/hdr"
cssh "curl -s -m 6 http://$SIP:8443/hdr" > "$W/orig.out" 2>/dev/null
check "originatingTLS: plain client request reaches the TLS server" grep -q "GET /hdr" "$W/orig.out"
FLOW_VM=np-client check "flow records the intercepted request" flow_has "tls intercepted" --port 443
FLOW_VM=np-client check "flow records the rewrite" flow_has "X-Via added" --port 443

echo "== back to observe =="
observe
check "observe again: port 8080 open" is 200 :8080/ok
check "observe again: GET /secret 200" is 200 /secret

echo "== flow history, learn, replay, detection =="
edges() { "$M" flow edges --vm "$1" -o json 2>/dev/null; }
edge_has() {
    local vm=$1 filter=$2
    for _ in 1 2 3 4 5; do
        edges "$vm" | jq -e "[.items[] | select($filter)] | length > 0" >/dev/null && return
        sleep 2
    done
    edges "$vm" | jq -c '.items[] | select(.port < 2000 or .port > 2030)
        | {s: (.src_vm // .src), d: (.dst_vm // .dst), p: .port, dir: .direction, v: .verdict, l7: [.l7[]? | "\(.request) ×\(.count)"]}' \
        | head -12 | sed 's/^/      /' >&3
    return 1
}
check "history: client → server :80 with GET /ok" edge_has np-server \
    '.src_vm == "np-client" and .dst_vm == "np-server" and .port == 80 and any(.l7[]?; .request | test("^GET .*/ok$"))'
check "history: denied 8080 edge kept" edge_has np-server '.port == 8080 and .verdict == "DROPPED"'
check "history: proxied 443 has status and latency" edge_has np-client \
    '.port == 443 and any(.l7[]?; (.status["2xx"] // 0) > 0 and .latency_n > 0)'
"$M" netpol learn --vm np-server > "$W/learned.yaml" 2> "$W/learned.err"
check "learn: policy for np-server" grep -q "app: np-server" "$W/learned.yaml"
grep -q "app: np-server" "$W/learned.yaml" || sed 's/^/      /' "$W/learned.err" "$W/learned.yaml"
learned_80() { grep -q 'app: np-client' "$W/learned.yaml" && grep -qE "port: ['\"]?80['\"]?$" "$W/learned.yaml"; }
check "learn: admits np-client on 80" learned_80
check "learn: learned YAML validates" "$M" netpol validate -f "$W/learned.yaml"
"$M" netpol replay -f "$W/learned.yaml" > "$W/replay.out" 2>&1
check "replay: learned policy breaks nothing observed" grep -q "Safe:" "$W/replay.out"
grep -q "Safe:" "$W/replay.out" || sed 's/^/      /' "$W/replay.out"
cat > "$W/narrow.yaml" <<'Y'
apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: np-realvm-proxy
specs:
  - endpointSelector:
      matchLabels: {app: np-client}
    egress:
      - toEndpoints: [{matchLabels: {app: np-server}}]
        toPorts: [{ports: [{port: "8443", protocol: TCP}]}]
  - endpointSelector:
      matchLabels: {app: np-server}
    ingress:
      - fromEndpoints: [{matchLabels: {app: np-client}}]
        toPorts: [{ports: [{port: "8443", protocol: TCP}]}]
Y
"$M" netpol replay -f "$W/narrow.yaml" > "$W/narrow.out" 2>&1
narrow_breaks_443() { grep -q "Would break" "$W/narrow.out" && grep -q "np-server tcp/443" "$W/narrow.out"; }
check "replay: dropping 443 from np-realvm-proxy would break client → server :443" narrow_breaks_443
narrow_breaks_443 || sed 's/^/      /' "$W/narrow.out"
cssh "for p in \$(seq 2000 2030); do timeout 1 bash -c '</dev/tcp/$SIP/'\$p 2>/dev/null; done; true"
alert_has() {
    for _ in 1 2 3 4 5; do
        "$M" flow alerts 2>/dev/null | grep -q "$1" && return
        sleep 2
    done
    return 1
}
check "detect: port scan from np-client" alert_has port_scan
persisted() {
    for _ in $(seq 40); do
        sudo -n test -s /var/lib/machina/bpf/flow-history.json && return
        sleep 2
    done
    return 1
}
check "history persisted to disk" persisted

echo "== quarantine =="
# An established client → server SSH session that ticks twice a second.
scp -q -i "$W/key" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR "$W/key" "np@$CIP:/tmp/k"
cssh "rm -f /tmp/ticks; setsid nohup ssh -i /tmp/k -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ServerAliveInterval=1 -o ServerAliveCountMax=600 np@$SIP 'while :; do echo t; sleep 0.5; done' > /tmp/ticks 2>/dev/null < /dev/null & disown"
ticks() { cssh "wc -l < /tmp/ticks" 2>/dev/null; }
growing() { local a b; a=$(ticks); sleep 2; b=$(ticks); [[ -n "$a" && "$b" -gt "$a" ]]; }
check "quarantine: long-lived client → server session is flowing" growing
check "quarantine np-server for 10m (host SSH allowed)" "$M" vm quarantine np-server --for 10m --allow-host-ssh --reason "realvm test"
check "quarantine: listed with time left" bash -c "'$M' netpol quarantines | grep -q 'np-server.*ingress host tcp/22'"
check "quarantine: open session cut" bash -c "! { a=\$(ssh -i '$W/key' -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR np@$CIP 'wc -l < /tmp/ticks'); sleep 3; b=\$(ssh -i '$W/key' -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR np@$CIP 'wc -l < /tmp/ticks'); [ \"\$b\" -gt \"\$a\" ]; }"
check "quarantine: new client → server :80 dropped (observe mode)" is 000 /ok
check "quarantine: host → server :80 dropped" bash -c "! curl -fs -m 3 -o /dev/null http://$SIP/ok"
check "quarantine: host SSH exception works" sssh true
check "quarantine: server egress cut" bash -c "! ssh -i '$W/key' -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR np@$SIP 'ping -c1 -W2 192.168.122.1'"
check "quarantine: client unaffected" cssh "ping -c1 -W2 192.168.122.1"
check "quarantine: DROPPED flows say quarantine" flow_has quarantine --verdict DROPPED
check "release" "$M" vm release np-server
check "released: client → server :80 back" is 200 /ok
check "released: nothing listed" bash -c "! '$M' netpol quarantines 2>/dev/null | grep -q np-server"

check "quarantine np-server for 6s" "$M" vm quarantine np-server --for 6s
check "short quarantine: :80 dropped" is 000 /ok
sleep 7
check "short quarantine lifted by the kernel at the deadline" is 200 /ok
gone() { for _ in $(seq 15); do "$M" netpol quarantines 2>/dev/null | grep -q np-server || return 0; sleep 1; done; return 1; }
check "short quarantine forgotten by bpfd" gone

check "quarantine np-server for 10m before a bpfd restart" "$M" vm quarantine np-server --for 10m --allow-host-ssh
check "restart machina-bpfd" sudo -n systemctl restart machina-bpfd
for _ in $(seq 20); do bpfd '{"op":"vm_quarantines"}' 2>/dev/null | grep -q np-server && break; sleep 1; done
check "restart: quarantine restored" bash -c "'$M' netpol quarantines | grep -q np-server"
held() { for _ in $(seq 10); do is 000 /ok && return; sleep 1; done; return 1; }
check "restart: still dropping" held
check "release after restart" "$M" vm release np-server
back() { for _ in $(seq 10); do is 200 /ok && return; sleep 1; done; return 1; }
check "restart: traffic back after release" back

# A VM with no policy at all: its taps are programmed for the quarantine only.
"$M" netpol delete np-realvm >/dev/null 2>&1
"$M" netpol delete np-realvm-proxy >/dev/null 2>&1
check "no policies: client reaches the gateway" cssh "ping -c1 -W2 192.168.122.1"
check "quarantine np-client with no policy" "$M" vm quarantine np-client --for 5m --allow-host-ssh
check "no policies: client egress cut" bash -c "! ssh -i '$W/key' -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR np@$CIP 'ping -c1 -W2 192.168.122.1'"
check "no policies: host SSH exception works" cssh true
check "release np-client" "$M" vm release np-client
check "no policies: client egress back" cssh "ping -c1 -W2 192.168.122.1"

echo "passed=$P failed=$F"
[[ $F -eq 0 ]]

#!/usr/bin/env bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0
#
# VM network policy against two real libvirt VMs (Debian cloud image on the
# `default` NAT network): observe first, then a short enforcement lease.
# Creates np-client / np-server and deletes them, the policy and the base
# image on exit; the edge is always returned to observe.
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
LEASE="${LEASE:-180}"
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
    "$M" netpol delete np-realvm >/dev/null 2>&1
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
runcmd:
  - [mkdir, -p, /srv]
  - [sh, -c, "echo ok > /srv/ok; echo secret > /srv/secret"]
  - [systemd-run, --unit, np80, python3, -m, http.server, "80", --directory, /srv]
  - [systemd-run, --unit, np8080, python3, -m, http.server, "8080", --directory, /srv]
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
cssh() { ssh -i "$W/key" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ConnectTimeout=5 -o LogLevel=ERROR "np@$CIP" "$@"; }
for _ in $(seq 90); do cssh true 2>/dev/null && break; sleep 2; done
check "ssh into client" cssh true
for _ in $(seq 60); do curl -fs -m 2 "http://$SIP/ok" >/dev/null && curl -fs -m 2 "http://$SIP:8080/ok" >/dev/null && break; sleep 2; done
check "server answers on 80 and 8080" curl -fs -m 2 "http://$SIP:8080/ok"

# HTTP status of a request from the client to the server ("000" = no answer).
get() { local path=$1; shift; cssh "curl -s -m 4 -o /dev/null -w '%{http_code}' $* http://$SIP$path" 2>/dev/null; }
is() { local want=$1; shift; [[ "$(get "$@")" == "$want" ]]; }
taps_programmed() {
    bpfd '{"op":"vm_edge_status"}' | python3 -c 'import json,sys; sys.exit(0 if len(json.load(sys.stdin).get("taps", [])) >= 2 else 1)'
}
flow_has() {
    local pat=$1; shift
    for _ in 1 2 3 4 5; do
        "$M" flow observe --vm np-server --last 300 "$@" > "$W/flows" 2>&1
        grep -q -- "$pat" "$W/flows" && return
        sleep 2
    done
    { echo "      flow observe --vm np-server $*:"; tail -n 8 "$W/flows"; echo "      unfiltered:"; "$M" flow observe --last 8 2>&1; } | sed 's/^/      /' >&3
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

echo "== back to observe =="
observe
check "observe again: port 8080 open" is 200 :8080/ok
check "observe again: GET /secret 200" is 200 /secret

echo "passed=$P failed=$F"
[[ $F -eq 0 ]]

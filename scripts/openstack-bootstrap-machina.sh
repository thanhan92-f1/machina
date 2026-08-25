#!/usr/bin/env bash
# One-shot OpenStack bootstrap for Machina on a Packstack-partial / RDO host.
#
# Idempotent where possible. Combines httpd/Keystone fixes, wire-cloud,
# minimal Nova+Glance, Keystone endpoint registration, OVN chassis, Nova cell
# mapping, compute, and optional catalog seeding.
#
# Run on the hypervisor as root (after Packstack left configs and keystonerc):
#   sudo ./scripts/openstack-bootstrap-machina.sh
#   sudo ./scripts/openstack-bootstrap-machina.sh /root/keystonerc_admin 185.165.240.5
#   sudo ./scripts/openstack-bootstrap-machina.sh /root/keystonerc_admin 185.165.240.5 --fake-compute
#
# From your laptop (copy script to host or use repo checkout there):
#   ssh sus@HOST 'sudo bash -s -- /root/keystonerc_admin HOST' \
#     < scripts/openstack-bootstrap-machina.sh
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

RC_FILE="/root/keystonerc_admin"
CONTROLLER_HOST=""
CLOUD_NAME="packstack"
SEED_CATALOG=1
SETUP_OVN=1
FIX_HTTPD=1
RESTART_MACHINA=1
COMPUTE_DRIVER="auto" # auto | libvirt | fake
ALLOW_LIBVIRT_CONFLICT=0
NO_FAKE_FALLBACK=0
CHECK_LIBVIRT_ONLY=0

usage() {
  sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'
  echo ""
  echo "Options:"
  echo "  --skip-seed          Do not create m1.tiny / cirros / private network"
  echo "  --skip-ovn           Skip OVN controller + neutron-ovn-agent setup"
  echo "  --skip-httpd-fix     Skip port-80 httpd conflict remediation"
  echo "  --no-restart-machina Do not restart machina-daemon at the end"
  echo "  --fake-compute       Force nova compute_driver=fake.FakeDriver"
  echo "  --libvirt-compute    Force libvirt.LibvirtDriver (may fail if libvirt has VMs)"
  echo "  --allow-libvirt-conflict  Try libvirt compute even if virsh lists domains"
  echo "  --no-fake-fallback   Do not fall back to fake.FakeDriver if libvirt fails"
  echo "  --check-libvirt-conflict  Print libvirt/Nova conflict and exit (no changes)"
  echo "  -h, --help           Show this help"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --skip-seed) SEED_CATALOG=0 ;;
    --skip-ovn) SETUP_OVN=0 ;;
    --skip-httpd-fix) FIX_HTTPD=0 ;;
    --no-restart-machina) RESTART_MACHINA=0 ;;
    --fake-compute) COMPUTE_DRIVER="fake" ;;
    --libvirt-compute) COMPUTE_DRIVER="libvirt" ;;
    --allow-libvirt-conflict) ALLOW_LIBVIRT_CONFLICT=1 ;;
    --no-fake-fallback) NO_FAKE_FALLBACK=1 ;;
    --check-libvirt-conflict) CHECK_LIBVIRT_ONLY=1 ;;
    --) shift; break ;;
    -*) echo "Unknown option: $1" >&2; usage >&2; exit 1 ;;
    *)
      if [[ "$RC_FILE" == "/root/keystonerc_admin" && ( -f "$1" || "$1" == *keystone* ) ]]; then
        RC_FILE="$1"
      elif [[ -z "$CONTROLLER_HOST" ]]; then
        CONTROLLER_HOST="$1"
      else
        echo "Unexpected argument: $1" >&2; exit 1
      fi
      ;;
  esac
  shift
done

while [[ $# -gt 0 ]]; do
  if [[ -z "$CONTROLLER_HOST" ]]; then
    CONTROLLER_HOST="$1"
  else
    echo "Unexpected argument: $1" >&2; exit 1
  fi
  shift
done

log() { echo "==> $*"; }
warn() { echo "WARNING: $*" >&2; }
die() { echo "ERROR: $*" >&2; exit 1; }

[[ "$(id -u)" -eq 0 ]] || die "Run as root on the hypervisor (sudo $0 …)"

[[ -f "$RC_FILE" ]] || die "Missing RC file: $RC_FILE"

read_conf() {
  local file="$1" key="$2"
  grep -E "^${key}=" "$file" 2>/dev/null | head -1 | cut -d= -f2- | tr -d ' "' || true
}

os_env() {
  # shellcheck disable=SC1090
  source "$RC_FILE"
  export OS_CLOUD="${OS_CLOUD:-$CLOUD_NAME}"
  export OS_CLIENT_CONFIG_FILE="${OS_CLIENT_CONFIG_FILE:-/etc/openstack/clouds.yaml}"
  export OS_REGION_NAME="${OS_REGION_NAME:-RegionOne}"
}

fix_httpd_port80_conflict() {
  [[ "$FIX_HTTPD" -eq 1 ]] || return 0
  log "httpd: avoid port 80 conflict (Keystone/Nova WSGI on 5000/8774)"
  local f
  for f in 15-default-80.conf 15-horizon_vhost.conf 10-aodh_wsgi.conf 10-gnocchi_wsgi.conf; do
    local p="/etc/httpd/conf.d/$f"
    if [[ -f "$p" && ! -f "${p}.disabled" ]]; then
      mv "$p" "${p}.disabled"
      log "  disabled $f"
    fi
  done
  if [[ -f /etc/httpd/conf/ports.conf ]] && grep -q '^Listen 80' /etc/httpd/conf/ports.conf; then
    sed -i 's/^Listen 80/#Listen 80/' /etc/httpd/conf/ports.conf
    log "  commented Listen 80 in ports.conf"
  fi
  systemctl enable httpd 2>/dev/null || true
  systemctl restart httpd
  local code
  code="$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:5000/v3/ || echo 000)"
  [[ "$code" == "200" ]] || die "Keystone not reachable on :5000 (HTTP $code); check journalctl -u httpd"
  log "Keystone OK on :5000 (HTTP $code)"
}

ensure_keystone_endpoint() {
  local name="$1" type="$2" desc="$3" port="$4" path="${5:-}"
  local base="http://${CONTROLLER_HOST}:${port}${path}"
  if ! openstack service list -f value -c Name | grep -qx "$name"; then
    openstack service create --name "$name" --description "$desc" "$type"
    log "  created service $name"
  fi
  local sid
  sid="$(openstack service list -f value -c ID -c Name | awk -v n="$name" '$2==n {print $1; exit}')"
  local iface
  for iface in public internal admin; do
    if ! openstack endpoint list --service "$sid" -f value -c Interface -c URL 2>/dev/null | grep -q "^${iface}.*:${port}"; then
      openstack endpoint create --region "$OS_REGION_NAME" "$type" "$iface" "$base"
    fi
  done
}

ensure_service_user() {
  local name="$1" password="$2"
  [[ -n "$password" ]] || return 0
  openstack project show services >/dev/null 2>&1 || openstack project create services
  if openstack user show "$name" >/dev/null 2>&1; then
    openstack user set --password "$password" "$name"
  else
    openstack user create "$name" --domain default --password "$password"
    openstack role add --project services --user "$name" admin 2>/dev/null || true
  fi
}

ml2_uses_ovn() {
  [[ -f /etc/neutron/plugins/ml2/ml2_conf.ini ]] || return 1
  grep -E '^mechanism_drivers=' /etc/neutron/plugins/ml2/ml2_conf.ini 2>/dev/null | grep -q ovn
}

setup_ovn() {
  [[ "$SETUP_OVN" -eq 1 ]] || return 0
  ml2_uses_ovn || { log "ML2 not using OVN; skipping OVN setup"; return 0; }

  log "OVN: install host packages and register chassis"
  if ! command -v ovn-controller >/dev/null 2>&1; then
    dnf install -y rdo-ovn-host openstack-neutron-ovn-agent 2>/dev/null \
      || dnf install -y ovn24.03-host openstack-neutron-ovn-agent \
      || die "Could not install ovn host + neutron-ovn-agent packages"
  fi

  local host ip
  host="$(hostname -s)"
  ip="$CONTROLLER_HOST"
  if [[ "$ip" =~ ^127\. ]]; then
    ip="$(ip -4 route get 1.1.1.1 2>/dev/null | awk '{print $7; exit}' || true)"
  fi
  [[ -n "$ip" ]] || die "Could not determine controller IP for OVN encap"

  ovs-vsctl --may-exist add-br br-int
  ovs-vsctl --may-exist add-br br-ex 2>/dev/null || true
  ovs-vsctl set open . external-ids:system-id="$host"
  ovs-vsctl set open . "external-ids:ovn-remote=tcp:${ip}:6642"
  ovs-vsctl set open . external-ids:ovn-remote-probe-interval=60000
  ovs-vsctl set open . external-ids:ovn-encap-type=geneve
  ovs-vsctl set open . "external-ids:ovn-encap-ip=${ip}"
  ovs-vsctl set open . external-ids:ovn-bridge-mappings=physnet1:br-ex

  systemctl enable openvswitch ovn-northd ovn-controller neutron-ovn-agent 2>/dev/null || true
  systemctl start openvswitch 2>/dev/null || true
  systemctl restart ovn-northd ovn-controller neutron-ovn-agent 2>/dev/null || true

  sleep 3
  if ovn-sbctl list chassis 2>/dev/null | grep -q "name[[:space:]]*${host}"; then
    log "  OVN chassis registered: $host"
  else
    warn "OVN chassis not visible yet; check ovn-controller and /var/log/ovn/"
  fi

  systemctl restart neutron-ovn-agent neutron-server 2>/dev/null || true
}

libvirt_domains_present() {
  virsh list --all 2>/dev/null | awk 'NR>2 && $1+0==$1 {print}' | grep -q .
}

check_libvirt_compute_conflict() {
  [[ "$COMPUTE_DRIVER" == "fake" ]] && return 0
  if ! libvirt_domains_present; then
    return 0
  fi
  log "Libvirt domains present (Machina or other — Nova libvirt compute may refuse to start):"
  virsh list --all 2>/dev/null | awk 'NR>2 && $1+0==$1 {printf "  - %s (%s)\n", $2, $3}'
  if [[ "$ALLOW_LIBVIRT_CONFLICT" -eq 0 ]]; then
    die "Use --fake-compute, --allow-libvirt-conflict, or openstack-enable-libvirt-compute.sh on a clean host"
  fi
  warn "Continuing with --allow-libvirt-conflict"
}

set_compute_driver() {
  local driver="$1"
  local conf="/etc/nova/nova.conf"
  [[ -f "$conf" ]] || return 0
  case "$driver" in
    libvirt)
      sed -i 's/^#compute_driver=libvirt.LibvirtDriver/compute_driver=libvirt.LibvirtDriver/' "$conf"
      sed -i 's/^compute_driver=fake.FakeDriver/compute_driver=libvirt.LibvirtDriver/' "$conf"
      grep -q '^compute_driver=libvirt.LibvirtDriver' "$conf" \
        || echo 'compute_driver=libvirt.LibvirtDriver' >>"$conf"
      sed -i 's/^#virt_type=kvm/virt_type=kvm/' "$conf"
      ;;
    fake)
      sed -i 's/^compute_driver=libvirt.LibvirtDriver/compute_driver=fake.FakeDriver/' "$conf"
      grep -q '^compute_driver=fake.FakeDriver' "$conf" \
        || echo 'compute_driver=fake.FakeDriver' >>"$conf"
      ;;
  esac
}

setup_nova_compute() {
  check_libvirt_compute_conflict
  log "Nova: compute package and cell mapping"
  if ! rpm -q openstack-nova-compute >/dev/null 2>&1; then
    dnf install -y openstack-nova-compute
  fi

  os_env
  ensure_service_user nova "$(read_conf /etc/nova/nova.conf password)"
  ensure_service_user neutron "$(read_conf /etc/neutron/neutron.conf password)"
  ensure_service_user placement "$(read_conf /etc/placement/placement.conf password)"

  if command -v nova-manage >/dev/null 2>&1; then
    nova-manage cell_v2 discover_hosts --verbose 2>/dev/null || true
  fi

  local driver="$COMPUTE_DRIVER"
  if [[ "$driver" == "auto" ]]; then
    driver="libvirt"
    if virsh list --all 2>/dev/null | awk 'NR>2 && $1 != "Id" {print}' | grep -q .; then
      warn "libvirt domains exist; will try libvirt compute first, fall back to fake on failure"
    fi
  fi

  set_compute_driver "$driver"
  systemctl enable openstack-nova-conductor openstack-nova-scheduler openstack-nova-compute 2>/dev/null || true
  systemctl restart openstack-nova-conductor openstack-nova-scheduler 2>/dev/null || true
  systemctl restart openstack-nova-compute 2>/dev/null || true
  sleep 3

  if [[ "$driver" == "libvirt" ]] && ! systemctl is-active --quiet openstack-nova-compute 2>/dev/null; then
    if journalctl -u openstack-nova-compute -n 30 --no-pager 2>/dev/null | grep -qE 'existing instances|node identity'; then
      if [[ "$NO_FAKE_FALLBACK" -eq 1 ]]; then
        die "nova-compute failed with libvirt driver (--no-fake-fallback); see /var/log/nova/nova-compute.log"
      fi
      warn "nova-compute: libvirt startup failed; switching to fake.FakeDriver"
      set_compute_driver fake
      systemctl restart openstack-nova-compute
      sleep 2
    fi
  fi

  if ! systemctl is-active --quiet openstack-nova-compute 2>/dev/null; then
    if [[ "$driver" == "fake" || "$driver" == "libvirt" ]]; then
      warn "nova-compute not active; running openstack-repair-e2e-compute.sh"
      bash "${SCRIPT_DIR}/openstack-repair-e2e-compute.sh" "$RC_FILE" || true
    else
      warn "nova-compute not active; see /var/log/nova/nova-compute.log"
    fi
  else
    log "nova-compute is active"
    if command -v nova-manage >/dev/null 2>&1; then
      nova-manage cell_v2 discover_hosts --verbose 2>/dev/null || true
    fi
  fi
}

seed_catalog() {
  [[ "$SEED_CATALOG" -eq 1 ]] || return 0
  log "Catalog: m1.tiny, cirros-test, private network"
  os_env
  openstack flavor list -f value -c Name | grep -qx m1.tiny \
    || openstack flavor create --id 1 --ram 512 --disk 1 --vcpus 1 m1.tiny
  if ! openstack image list -f value -c Name | grep -qx cirros-test; then
    # mktemp, not a hardcoded /tmp path: this runs as root, and a fixed,
    # predictable filename in world-writable /tmp lets a local attacker
    # pre-plant a symlink/file there and have it seeded into Glance instead
    # of (or via a race, alongside) the real download.
    local cirros_img
    cirros_img="$(mktemp /tmp/cirros-XXXXXX.img)"
    curl -fsSL -o "$cirros_img" \
      https://download.cirros-cloud.net/0.6.2/cirros-0.6.2-x86_64-disk.img
    openstack image create cirros-test --disk-format qcow2 --container-format bare \
      --public --file "$cirros_img"
    rm -f "$cirros_img"
  fi
  if ! openstack network list -f value -c Name | grep -qx private; then
    openstack network create private
    openstack subnet create --network private --subnet-range 10.0.0.0/24 private-subnet
  fi
}

smoke_test() {
  log "Smoke test"
  os_env
  openstack token issue -f value -c id >/dev/null
  openstack server list >/dev/null
  openstack image list >/dev/null
  openstack network list >/dev/null
  openstack compute service list >/dev/null
  if ml2_uses_ovn; then
    openstack network agent list >/dev/null 2>&1 || warn "network agent list empty (OVN may still be starting)"
  fi
  log "CLI smoke test OK"
}

# ── main ─────────────────────────────────────────────────────────────────────

if [[ -z "$CONTROLLER_HOST" ]]; then
  # shellcheck disable=SC1090
  source "$RC_FILE"
  CONTROLLER_HOST="$(echo "${OS_AUTH_URL:-}" | sed -E 's#https?://([^/:]+).*#\1#')"
fi
[[ -n "$CONTROLLER_HOST" ]] || die "Pass controller IP as second argument"

log "Machina OpenStack bootstrap"
log "  RC: $RC_FILE  controller: $CONTROLLER_HOST  cloud: $CLOUD_NAME"

if [[ "$CHECK_LIBVIRT_ONLY" -eq 1 ]]; then
  check_libvirt_compute_conflict
  log "Libvirt conflict check OK (no changes made)"
  exit 0
fi

fix_httpd_port80_conflict

WIRE="$SCRIPT_DIR/openstack-wire-cloud.sh"
MINIMAL="$SCRIPT_DIR/openstack-minimal-services.sh"
[[ -f "$WIRE" ]] || die "Missing $WIRE (run from machina repo scripts/)"

bash "$WIRE" "$RC_FILE" "$CLOUD_NAME"

if [[ -f "$MINIMAL" ]]; then
  bash "$MINIMAL" "$RC_FILE" "$CONTROLLER_HOST"
else
  warn "openstack-minimal-services.sh not found; skipping"
fi

os_env
log "Keystone: glance / neutron / placement endpoints"
ensure_keystone_endpoint glance image "OpenStack Image" 9292
ensure_keystone_endpoint neutron network "OpenStack Networking" 9696
if curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:8778/" | grep -qE '200|300|401|404'; then
  ensure_keystone_endpoint placement placement "OpenStack Placement" 8778
fi

setup_ovn
setup_nova_compute
seed_catalog

bash "$WIRE" "$RC_FILE" "$CLOUD_NAME"

if [[ "$RESTART_MACHINA" -eq 1 ]] && systemctl list-unit-files machina-daemon.service >/dev/null 2>&1; then
  systemctl restart machina-daemon
  log "Restarted machina-daemon"
fi

smoke_test

log "Done."
echo ""
echo "Next:"
echo "  openstack server create --flavor m1.tiny --image cirros-test \\"
echo "    --nic net-id=\$(openstack network show private -f value -c id) test-vm"
echo "  curl -sk -b /tmp/cookies.txt https://${CONTROLLER_HOST}:5092/api/v1/openstack/status   # after UI login"
echo "  See docs/openstack-minimal.md"

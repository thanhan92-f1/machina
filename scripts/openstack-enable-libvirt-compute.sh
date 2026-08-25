#!/usr/bin/env bash
# Enable real libvirt Nova compute on a Machina hypervisor (no fake.FakeDriver).
#
# Prerequisites: no extra libvirt domains, or use a dedicated compute node (see docs).
#
# Usage (on hypervisor, as root):
#   sudo ./scripts/openstack-enable-libvirt-compute.sh
#   sudo ./scripts/openstack-enable-libvirt-compute.sh --check-only
#
set -euo pipefail

CHECK_ONLY=0
FORCE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --check-only) CHECK_ONLY=1 ;;
    --force) FORCE=1 ;;
    -h|--help)
      sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *) echo "Unknown option: $1" >&2; exit 1 ;;
  esac
  shift
done

log() { echo "==> $*"; }
die() { echo "ERROR: $*" >&2; exit 1; }

[[ "$(id -u)" -eq 0 ]] || die "Run as root"

libvirt_has_domains() {
  virsh list --all 2>/dev/null | awk 'NR>2 && $1+0==$1 {print $1}' | grep -q .
}

if libvirt_has_domains; then
  log "Libvirt domains on this host (Nova will refuse a new compute service):"
  virsh list --all 2>/dev/null | awk 'NR>2 && $1+0==$1 {printf "  - %s (%s)\n", $2, $3}'
  if [[ "$FORCE" -eq 0 ]]; then
    die "Stop or migrate Machina/libvirt VMs first, or use a second compute node. Re-run with --force to attempt anyway."
  fi
  warn_msg="--force: continuing despite existing libvirt domains"
  echo "WARNING: $warn_msg" >&2
fi

if [[ "$CHECK_ONLY" -eq 1 ]]; then
  log "Check OK (no blocking domains, or --force)"
  exit 0
fi

CONF=/etc/nova/nova.conf
[[ -f "$CONF" ]] || die "Missing $CONF"

HOST="$(hostname -s)"
sed -i "s/^compute_driver=.*/compute_driver=libvirt.LibvirtDriver/" "$CONF"
grep -q '^compute_driver=libvirt' "$CONF" || echo 'compute_driver=libvirt.LibvirtDriver' >>"$CONF"
sed -i 's/^#virt_type=kvm/virt_type=kvm/' "$CONF"
if grep -q '^host=.*-libvirt' "$CONF" 2>/dev/null; then
  sed -i "s/^host=.*/host=${HOST}/" "$CONF"
fi

log "Reset stale compute node state (fake → libvirt migration)"
mysql nova -e "DELETE FROM compute_nodes;" 2>/dev/null || true
mysql nova -e "DELETE FROM services WHERE binary='nova-compute';" 2>/dev/null || true
rm -rf /var/lib/nova/instances/compute_nodes
mkdir -p /var/lib/nova/instances/compute_nodes
chown nova:nova /var/lib/nova/instances/compute_nodes

if command -v nova-manage >/dev/null 2>&1; then
  CELL="$(mysql nova -N -e "SELECT uuid FROM cells WHERE name='default' LIMIT 1" 2>/dev/null || true)"
  if [[ -n "$CELL" ]]; then
    nova-manage cell_v2 delete_host --cell_uuid "$CELL" --host "$HOST" 2>/dev/null || true
  fi
  nova-manage cell_v2 discover_hosts --verbose 2>/dev/null || true
fi

systemctl enable openstack-nova-compute 2>/dev/null || true
systemctl restart openstack-nova-compute
sleep 5

if ! systemctl is-active --quiet openstack-nova-compute; then
  echo "--- nova-compute log ---" >&2
  tail -20 /var/log/nova/nova-compute.log >&2 || journalctl -u openstack-nova-compute -n 20 --no-pager >&2
  die "openstack-nova-compute failed to start"
fi

log "nova-compute active with libvirt driver"
if [[ -f /root/keystonerc_admin ]]; then
  # shellcheck disable=SC1091
  source /root/keystonerc_admin
  openstack compute service list
fi
log "Done. Run: VSPASS=… ./scripts/e2e-test.sh https://HOST:5092 USER --require-openstack-ssh"

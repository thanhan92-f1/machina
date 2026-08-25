#!/usr/bin/env bash
# Bootstrap minimal Nova (compute) + Glance on a Packstack-partial host for Machina testing.
#
# Assumes Keystone is already up (port 5000) and Packstack left httpd WSGI configs.
# Does NOT run full Packstack — only service users, endpoints, and unit fixes.
#
# Usage (on hypervisor, as root):
#   sudo ./scripts/openstack-minimal-services.sh
#   sudo ./scripts/openstack-minimal-services.sh /root/keystonerc_admin 212.8.252.194
#
set -euo pipefail

RC_FILE="${1:-/root/keystonerc_admin}"
CONTROLLER_HOST="${2:-}"
CLOUDS_YAML="${OS_CLIENT_CONFIG_FILE:-/etc/openstack/clouds.yaml}"
REGION="${OS_REGION_NAME:-RegionOne}"

log() { echo "==> $*"; }
die() { echo "ERROR: $*" >&2; exit 1; }

[[ -f "$RC_FILE" ]] || die "Missing RC file: $RC_FILE"
# shellcheck disable=SC1090
source "$RC_FILE"

if [[ -z "$CONTROLLER_HOST" ]]; then
  CONTROLLER_HOST="$(echo "$OS_AUTH_URL" | sed -E 's#https?://([^/:]+).*#\1#')"
fi
[[ -n "$CONTROLLER_HOST" ]] || die "Could not determine controller host; pass as second argument."

export OS_CLOUD="${OS_CLOUD:-packstack}"
export OS_CLIENT_CONFIG_FILE="$CLOUDS_YAML"
export OS_REGION_NAME="$REGION"

read_conf() {
  local file="$1" key="$2"
  grep -E "^${key}=" "$file" 2>/dev/null | head -1 | cut -d= -f2- | tr -d ' "' || true
}

ensure_user() {
  local name="$1" password="$2"
  if openstack user show "$name" >/dev/null 2>&1; then
    openstack user set --password "$password" "$name"
    log "Updated password for user $name"
  else
    openstack user create "$name" --domain default --password "$password"
    log "Created user $name"
  fi
  openstack role add --project services --user "$name" admin 2>/dev/null || true
}

ensure_compute_service() {
  local base="http://${CONTROLLER_HOST}:8774"
  if openstack service list -f value -c Name | grep -qx nova; then
    log "Nova compute service already registered"
  else
    openstack service create --name nova --description "OpenStack Compute" compute
    log "Created nova compute service"
  fi
  if ! openstack endpoint list --service compute -f value -c URL 2>/dev/null | grep -q "${base}"; then
    openstack endpoint create --region "$REGION" compute public "${base}/v2.1"
    openstack endpoint create --region "$REGION" compute internal "${base}/v2.1"
    openstack endpoint create --region "$REGION" compute admin "${base}/v2.1"
    log "Registered compute endpoints at ${base}/v2.1"
  else
    log "Compute endpoints already present"
  fi
}

fix_auth_url() {
  local conf="$1"
  [[ -f "$conf" ]] || return 0
  local auth_line
  auth_line="$(grep -E '^auth_url=' "$conf" | head -1 || true)"
  if [[ -n "$auth_line" && "$auth_line" != *"/v3"* ]]; then
    sed -i 's#^auth_url=http://\([^/]*\)/\?$#auth_url=http://\1/v3#' "$conf"
    sed -i 's#^auth_url=http://\([^/]*\)$#auth_url=http://\1/v3#' "$conf"
    log "Normalized auth_url in $conf"
  fi
}

ensure_memcached_in_glance() {
  local conf="/etc/glance/glance-api.conf"
  [[ -f "$conf" ]] || return 0
  if ! grep -q '^memcached_servers=' "$conf"; then
    if systemctl is-active memcached >/dev/null 2>&1; then
      echo "memcached_servers=localhost:11211" >>"$conf"
      log "Appended memcached_servers to glance-api.conf"
    fi
  fi
}

log "Controller: $CONTROLLER_HOST  region: $REGION  cloud: $OS_CLOUD"

NOVA_PW="$(read_conf /etc/nova/nova.conf password)"
GLANCE_PW="$(read_conf /etc/glance/glance-api.conf password)"
[[ -n "$NOVA_PW" ]] || die "Could not read nova password from /etc/nova/nova.conf"
[[ -n "$GLANCE_PW" ]] || die "Could not read glance password from /etc/glance/glance-api.conf"

openstack project show services >/dev/null 2>&1 || openstack project create services

ensure_user nova "$NOVA_PW"
ensure_user glance "$GLANCE_PW"
ensure_compute_service

fix_auth_url /etc/nova/nova.conf
fix_auth_url /etc/glance/glance-api.conf
ensure_memcached_in_glance

# Packstack serves Nova API via httpd on 8774; the eventlet unit conflicts on the same port.
if systemctl list-unit-files openstack-nova-api.service >/dev/null 2>&1; then
  systemctl stop openstack-nova-api.service 2>/dev/null || true
  systemctl mask openstack-nova-api.service 2>/dev/null || true
  log "Masked openstack-nova-api.service (use httpd WSGI on :8774)"
fi

systemctl enable httpd mariadb memcached rabbitmq-server 2>/dev/null || true
systemctl restart httpd 2>/dev/null || true

for unit in openstack-glance-api openstack-nova-conductor openstack-nova-scheduler; do
  if systemctl list-unit-files "${unit}.service" >/dev/null 2>&1; then
    systemctl enable "$unit" 2>/dev/null || true
    systemctl restart "$unit" 2>/dev/null || log "Warning: $unit did not start (optional for empty list)"
  fi
done

log "Smoke test (admin CLI)…"
if openstack server list -f value -c Name >/dev/null 2>&1; then
  log "Nova: openstack server list OK"
else
  log "Nova: server list failed (Keystone OK; check /var/log/nova/nova-api.log)"
fi
if openstack image list -f value -c Name >/dev/null 2>&1; then
  log "Glance: openstack image list OK"
else
  log "Glance: image list failed (check /var/log/glance/api.log)"
fi

log "Done. For full Machina E2E (OVN, compute, catalog), run:"
echo "  sudo ./scripts/openstack-bootstrap-machina.sh $RC_FILE $CONTROLLER_HOST"
echo "Or re-wire only:"
echo "  sudo /usr/local/share/machina/scripts/openstack-wire-cloud.sh $RC_FILE packstack"
echo "  sudo systemctl restart machina-daemon"
echo "  curl -sk https://127.0.0.1:5092/api/v1/openstack/status  # after UI login"

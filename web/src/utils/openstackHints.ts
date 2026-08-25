// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

export const WIRE_SCRIPT =
  'sudo /usr/local/share/machina/scripts/openstack-bootstrap-machina.sh /root/keystonerc_admin $(hostname -I | awk "{print $1}")\n# or: openstack-wire-cloud.sh + systemctl restart machina-daemon'

export const VERIFY_COMMANDS =
  'source /root/keystonerc_admin && openstack token issue\nopenstack server list'

export function openStackErrorHints(error: string | undefined): string[] {
  const e = (error || '').toLowerCase()
  const hints: string[] = []
  if (e.includes('html error page') || e.includes('</html>') || e.includes('<!doctype html')) {
    hints.push('A service returned HTML instead of JSON — often Neutron (:9696) or Nova when down.')
    hints.push('On the host: systemctl status neutron-server nova-api; journalctl -u neutron-server -n 40')
  }
  if (e.includes('connection refused') || e.includes('failed to establish')) {
    hints.push('Keystone/Nova may not be running — install or start Packstack/RDO on this host.')
    hints.push('Verify: curl -s http://127.0.0.1:5000/v3 or openstack token issue as admin.')
  }
  if (e.includes('503') || e.includes('neutron')) {
    hints.push('Neutron API may be down — create/list networks and instances need Neutron.')
    hints.push('Check: openstack network list && systemctl status neutron-server')
  }
  if (e.includes('glance') || e.includes('no active images')) {
    hints.push('Upload a qcow2 from Disk images, or run: openstack image list')
    hints.push('Images must be ACTIVE in Glance before you can boot an instance.')
  }
  if (e.includes('401') || e.includes('unauthorized') || e.includes('authentication')) {
    hints.push('Check /etc/openstack/clouds.yaml credentials match your keystonerc_admin.')
    hints.push('Re-run the wire script after updating /root/keystonerc_admin.')
  }
  if (e.includes('timeout') || e.includes('timed out')) {
    hints.push('Increase connect_timeout_secs in /etc/machina/config.toml or fix firewall to port 5000.')
  }
  if (e.includes('fake') || e.includes('did not reach active') || e.includes('entered error')) {
    hints.push('Nova may be using fake.FakeDriver — instances get IPs but no real VM. Enable libvirt compute or add a second compute node (docs/openstack-minimal.md).')
    hints.push('On host: sudo ./scripts/openstack-enable-libvirt-compute.sh --check-only')
  }
  if (e.includes('compute') && (e.includes('unavailable') || e.includes('nova off'))) {
    hints.push('Start openstack-nova-api and openstack-nova-compute; re-run openstack-bootstrap-machina.sh if needed.')
  }
  if (e.includes('operation failed') || e.includes('operation_failed')) {
    hints.push('Read the technical details below, then fix the failing OpenStack service on the host.')
    hints.push('Restart machina-daemon after fixing clouds.yaml or keystonerc.')
  }
  if (hints.length === 0) {
    hints.push('On the host: source /root/keystonerc_admin && openstack token issue')
    hints.push('Then: sudo systemctl restart machina-daemon and Test connection in Settings.')
  }
  return hints
}

'use strict';

/**
 * Resolve live platform host / VM UUIDs after login.
 * Prefer MACHINA_HOST_ID / MACHINA_PLATFORM_HOST_ID / MACHINA_PLATFORM_VM_ID;
 * otherwise pick from controller inventory (first host; VM matching cfg.vmName).
 */

const P = '/api/v1/platform/controller';

async function resolveIds(api, cfg = {}) {
  let hostId =
    cfg.hostId ||
    process.env.MACHINA_HOST_ID ||
    process.env.MACHINA_PLATFORM_HOST_ID ||
    '';
  let platformVmId = cfg.platformVmId || process.env.MACHINA_PLATFORM_VM_ID || '';
  const vmName = cfg.vmName || process.env.MACHINA_VM_NAME || '';

  if (!hostId) {
    const r = await api('GET', `${P}/api/v1/hosts`);
    if (r.status < 200 || r.status >= 400) {
      throw new Error(`hosts list ${r.status}`);
    }
    const body = JSON.parse(r.body || '[]');
    const list = Array.isArray(body) ? body : body.items || [];
    if (!list.length) throw new Error('no platform hosts — set MACHINA_HOST_ID');
    hostId = list[0].id;
  }

  if (!platformVmId && vmName) {
    const r = await api('GET', `${P}/api/v1/vms`);
    if (r.status >= 200 && r.status < 400) {
      const body = JSON.parse(r.body || '[]');
      const list = Array.isArray(body) ? body : body.items || [];
      const hit = list.find((v) => v.name === vmName);
      if (hit) platformVmId = hit.id;
    }
  }

  return { hostId, platformVmId, vmName };
}

module.exports = { resolveIds, P };

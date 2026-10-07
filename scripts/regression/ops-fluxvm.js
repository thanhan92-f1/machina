#!/usr/bin/env node
// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

'use strict';

/**
 * FluxVM end to end through machina-daemon and the controller, on one host:
 *   VM A (QEMU, default storage, host bridge, direct kernel boot, guest agent):
 *   serial console, agent console, snapshots + revert, hot-add vCPU/memory,
 *   extra NIC, backup + restore.
 *   VM B (QEMU, shared raw disk, netns NIC): NIC hot-add refused, daemon live
 *   migration to the same host,
 *   controller inventory row, controller vm.migrate host-to-itself, HA
 *   re-create on the same host (POST …/fluxvm/recover).
 *
 * Needs `[fluxvm] enabled` on the daemon, MACHINA_FLUXVM_URL on the agent and a
 * kernel/initrd + image on the host. The shared disk is copied on the host
 * (locally when MACHINA_BASE_URL is loopback, else over ssh FLUXVM_SSH).
 */

const { execSync } = require('child_process');
const WebSocket = require('ws');
const { loadConfig } = require('./lib/config');
const { createApi } = require('./lib/api');
const { createLogger } = require('./lib/log');

const cfg = loadConfig();
const { api, login } = createApi(cfg);
const log = createLogger(cfg.resultsDir, 'ops-fluxvm');
const P = '/api/v1/platform/controller';

const IMAGE = process.env.FLUXVM_IMAGE || '/var/lib/fluxvm/images/linux-agent.raw';
const KERNEL = process.env.FLUXVM_QEMU_KERNEL || '/var/lib/fluxvm/kernels/bionic-vmlinuz-4.15';
const INITRD = process.env.FLUXVM_QEMU_INITRD || '/var/lib/fluxvm/kernels/bionic-initrd-4.15';
const KARGS = process.env.FLUXVM_KERNEL_ARGS || 'console=ttyS0 root=/dev/vda rw';
const SHARED_DIR = process.env.FLUXVM_SHARED_DIR || '/var/lib/fluxvm/shared';
const BRIDGE = process.env.FLUXVM_BRIDGE || 'virbr0';
const SKIP_PLATFORM = process.env.FLUXVM_SKIP_PLATFORM === '1';
const SUFFIX = Date.now().toString(36);
const A = `mfx-a-${SUFFIX}`;
const B = `mfx-b-${SUFFIX}`;
const SHARED_DISK = `${SHARED_DIR}/${B}.raw`;
const LOCAL = ['127.0.0.1', 'localhost', '::1'].includes(cfg.host);
const SSH = process.env.FLUXVM_SSH || `${cfg.username}@${cfg.host}`;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const ok = (s) => s >= 200 && s < 300;
const q = (name, suffix = '') => `/api/v1/vms/${encodeURIComponent(name)}${suffix}?backend=fluxvm`;

function onHost(cmd) {
  const full = LOCAL ? cmd : `ssh -o BatchMode=yes ${SSH} ${JSON.stringify(cmd)}`;
  return execSync(full, { stdio: ['ignore', 'pipe', 'pipe'] }).toString();
}

function json(r) {
  try {
    return JSON.parse(r.body);
  } catch {
    return null;
  }
}

async function must(method, path, body) {
  const r = await api(method, path, body);
  if (!ok(r.status)) throw new Error(`${method} ${path} → ${r.status} ${String(r.body).slice(0, 200)}`);
  return json(r);
}

async function details(name) {
  return must('GET', q(name));
}

async function waitFor(what, fn, timeoutMs = 120000, everyMs = 2000) {
  const end = Date.now() + timeoutMs;
  let last;
  while (Date.now() < end) {
    try {
      last = await fn();
      if (last) return last;
    } catch (e) {
      if (e.final) throw e;
      last = e.message;
    }
    await sleep(everyMs);
  }
  throw new Error(`timed out waiting for ${what} (last: ${String(JSON.stringify(last)).slice(0, 160)})`);
}

const waitState = (name, state, ms) =>
  waitFor(`${name} ${state}`, async () => ((await details(name)).state === state ? state : null), ms);

async function wsToken() {
  const j = await must('POST', '/api/v1/ws-token', {});
  if (!j?.token) throw new Error('no ws token');
  return j.token;
}

function wsUrl(path) {
  return `${cfg.baseUrl.replace(/^http/, 'ws')}${path}`;
}

/** Opens `path`, sends `input` after `delayMs`, resolves with everything received. */
function wsExchange(path, { input, delayMs = 1500, waitMs = 8000, until } = {}) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(wsUrl(path), { rejectUnauthorized: !cfg.insecureTls });
    let buf = '';
    let opened = false;
    const finish = (err) => {
      try {
        ws.close();
      } catch {
        /* closed */
      }
      if (err) reject(err);
      else resolve(buf);
    };
    const timer = setTimeout(() => finish(), waitMs);
    ws.on('open', () => {
      opened = true;
      if (input != null) setTimeout(() => ws.send(Buffer.from(input)), delayMs);
    });
    ws.on('message', (d) => {
      buf += Buffer.isBuffer(d) ? d.toString('utf8') : String(d);
      if (until && until.test(buf)) {
        clearTimeout(timer);
        finish();
      }
    });
    ws.on('error', (e) => {
      clearTimeout(timer);
      finish(new Error(`ws error ${e.message}`));
    });
    ws.on('close', (code) => {
      if (!opened) {
        clearTimeout(timer);
        finish(new Error(`closed before open (${code})`));
      }
    });
  });
}

async function platformVm(name) {
  const r = await api('GET', `${P}/api/v1/vms?limit=1000`);
  if (!ok(r.status)) throw new Error(`platform vms ${r.status}`);
  const j = json(r);
  const items = Array.isArray(j) ? j : j?.items || j?.vms || [];
  return items.find((v) => v.name === name) || null;
}

async function waitTask(taskId, timeoutMs = 600000) {
  return waitFor(
    `task ${taskId}`,
    async () => {
      const t = await must('GET', `${P}/api/v1/tasks/${taskId}`);
      if (t.status === 'completed') return t;
      if (t.status === 'failed' || t.status === 'cancelled') {
        throw Object.assign(new Error(`task ${t.status}: ${t.error || t.message || ''}`), { final: true });
      }
      return null;
    },
    timeoutMs,
    3000,
  );
}

(async () => {
  let passed = 0;
  let fail = 0;
  const failed = [];
  const step = async (name, fn) => {
    try {
      const note = await fn();
      log.append({ kind: 'FLUXVM', api: name, ok: true, note: String(note ?? 'ok').slice(0, 180) });
      passed++;
      return true;
    } catch (e) {
      log.append({ kind: 'FLUXVM', api: name, ok: false, note: e.message.slice(0, 300) });
      failed.push(name);
      fail++;
      return false;
    }
  };

  await login({ retries: 5, waitMs: 65000 });

  await step('fluxvm-status', async () => {
    const j = await must('GET', '/api/v1/fluxvm/status');
    if (!j.enabled || !j.reachable) throw new Error(JSON.stringify(j).slice(0, 160));
    return `version=${j.version ?? '?'}`;
  });

  // --- VM A: guest features ---------------------------------------------------
  const createdA = await step('create-kernel-agent', async () => {
    await must('POST', '/api/v1/vms', {
      name: A,
      vcpus: 1,
      memory_mb: 1024,
      disk_gb: 0,
      backend: 'fluxvm',
      fluxvm_backend: 'qemu',
      fluxvm_image: IMAGE,
      fluxvm_bridge: BRIDGE,
      fluxvm_kernel: KERNEL,
      fluxvm_initrd: INITRD,
      fluxvm_kernel_args: KARGS,
    });
    const d = await details(A);
    if (d.state !== 'running') await must('POST', q(A, '/start'));
    await waitState(A, 'running');
    return `${A} running`;
  });

  if (createdA) {
    await step('serial-console', async () => {
      const token = await wsToken();
      const out = await wsExchange(`/ws/v1/console/${encodeURIComponent(A)}?token=${encodeURIComponent(token)}&backend=fluxvm`, {
        input: '\r',
        waitMs: 6000,
      });
      return `${out.length} bytes`;
    });

    await step('agent-console', async () => {
      const token = await wsToken();
      const path = `/ws/v1/fluxvm-console/${encodeURIComponent(A)}?token=${encodeURIComponent(token)}&cols=100&rows=30`;
      const out = await waitFor(
        'agent console echo',
        async () => {
          const o = await wsExchange(path, { input: 'echo MNX$((6*7))\n', delayMs: 1500, waitMs: 10000, until: /MNX42/ });
          return /MNX42/.test(o) ? o : null;
        },
        150000,
        5000,
      );
      return `shell answered (${out.length} bytes)`;
    });

    await step('snapshot-create-list', async () => {
      await must('POST', q(A, '/snapshots'), { name: 's1' });
      const list = await must('GET', q(A, '/snapshots'));
      const items = Array.isArray(list) ? list : list?.items || [];
      if (!items.some((s) => s.name === 's1')) throw new Error(`s1 missing: ${JSON.stringify(items).slice(0, 160)}`);
      return `${items.length} snapshot(s)`;
    });

    await step('hotplug-vcpu-memory', async () => {
      await must('POST', `/api/v1/vms/${encodeURIComponent(A)}/vcpus/2?backend=fluxvm`);
      await must('POST', `/api/v1/vms/${encodeURIComponent(A)}/memory/1536?backend=fluxvm`);
      const d = await details(A);
      const mem = d.memory_mb ?? d.memory ?? 0;
      if (d.vcpus !== 2) throw new Error(`vcpus=${d.vcpus}`);
      if (mem !== 1536) throw new Error(`memory=${mem}`);
      return 'vcpus=2 memory=1536';
    });

    await step('hotplug-shrink-refused', async () => {
      const r = await api('POST', `/api/v1/vms/${encodeURIComponent(A)}/vcpus/1?backend=fluxvm`);
      if (ok(r.status)) throw new Error('shrink accepted');
      return `${r.status}`;
    });

    await step('nic-attach-detach', async () => {
      const before = (await details(A)).interfaces?.length ?? 0;
      const r = await must('POST', q(A, '/nic/attach'), { network: BRIDGE, model: 'virtio' });
      if (!r?.mac) throw new Error(`no mac in ${JSON.stringify(r)}`);
      const d = await details(A);
      const added = (d.interfaces || []).find((i) => i.mac_address === r.mac);
      if (!added || d.interfaces.length !== before + 1) throw new Error(JSON.stringify(d.interfaces).slice(0, 200));
      await must('POST', q(A, `/nic/detach/${encodeURIComponent(r.mac)}`));
      const after = (await details(A)).interfaces?.length ?? 0;
      if (after !== before) throw new Error(`interfaces ${after} != ${before}`);
      return `added+removed ${r.mac}`;
    });

    await step('stop', async () => {
      await must('POST', q(A, '/stop'));
      await waitState(A, 'shutoff', 90000);
    });

    await step('snapshot-revert', async () => {
      await must('POST', q(A, '/snapshots/s1/revert'));
      await waitState(A, 'running', 90000);
      await must('POST', q(A, '/stop'));
      await waitState(A, 'shutoff', 90000);
      return 'relaunched from s1';
    });

    await step('snapshot-delete', async () => {
      await must('DELETE', q(A, '/snapshots/s1'));
      const list = await must('GET', q(A, '/snapshots'));
      const items = Array.isArray(list) ? list : list?.items || [];
      if (items.some((s) => s.name === 's1')) throw new Error('s1 still listed');
    });

    let backupId = '';
    await step('backup-create-list', async () => {
      const r = await must('POST', '/api/v1/backups', { vm_name: A, backend: 'fluxvm', compress: false });
      backupId = r.backup_id || r.backup?.id || '';
      const item = await waitFor('backup listed', async () => {
        const l = await must('GET', `/api/v1/backups?backend=fluxvm&vm=${encodeURIComponent(A)}`);
        return (l || []).find((b) => b.id === backupId || (!backupId && b.vm_filter === A)) || null;
      });
      backupId = item.id;
      if (item.backend !== 'fluxvm') throw new Error(`backend=${item.backend}`);
      return `${item.id} ${item.size}`;
    });

    await step('backup-restore', async () => {
      if (!backupId) throw new Error('no backup');
      await must('POST', '/api/v1/backups/restore', { backup_id: backupId, backend: 'fluxvm', vm_name: A });
      return backupId;
    });

    await step('backup-delete', async () => {
      if (!backupId) throw new Error('no backup');
      await must('DELETE', `/api/v1/backups/${encodeURIComponent(backupId)}?backend=fluxvm`);
    });
  }

  // --- VM B: shared disk, migration, HA ----------------------------------------
  const createdB = await step('create-shared-disk', async () => {
    onHost(`sudo mkdir -p ${SHARED_DIR} && sudo cp --reflink=auto ${IMAGE} ${SHARED_DISK}`);
    await must('POST', '/api/v1/vms', {
      name: B,
      vcpus: 1,
      memory_mb: 1024,
      disk_gb: 0,
      backend: 'fluxvm',
      fluxvm_backend: 'qemu',
      fluxvm_image: SHARED_DISK,
      fluxvm_kernel: KERNEL,
      fluxvm_initrd: INITRD,
      fluxvm_kernel_args: KARGS,
      fluxvm_shared_disk: true,
    });
    if ((await details(B)).state !== 'running') await must('POST', q(B, '/start'));
    await waitState(B, 'running');
    const lock = onHost(`test -f ${SHARED_DISK}.fluxvm-lock && echo yes || echo no`).trim();
    if (lock !== 'yes') throw new Error('no disk lock');
    return `${B} on ${SHARED_DISK}`;
  });

  if (createdB) {
    await step('nic-refused-on-netns', async () => {
      const r = await api('POST', q(B, '/nic/attach'), { network: BRIDGE, model: 'virtio' });
      if (ok(r.status)) throw new Error('accepted');
      if (!/netns/.test(r.body)) throw new Error(`${r.status} ${String(r.body).slice(0, 160)}`);
      return `${r.status}`;
    });

    await step('daemon-migrate-loopback', async () => {
      const before = (await details(B)).uuid;
      await must('POST', q(B, '/migrate'), { dest_uri: 'local', live: true });
      const d = await waitState(B, 'running', 60000).then(() => details(B));
      if (!d.uuid || d.uuid === before) throw new Error(`uuid unchanged (${before})`);
      return `${before.slice(0, 8)} → ${d.uuid.slice(0, 8)}`;
    });

    if (!SKIP_PLATFORM) {
      let row = null;
      await step('controller-inventory', async () => {
        row = await waitFor(
          'controller row running',
          async () => {
            const v = await platformVm(B);
            return v && (v.observed_state ?? v.state) === 'running' ? v : null;
          },
          240000,
          5000,
        );
        if (row.inventory_source && row.inventory_source !== 'fluxvm') throw new Error(`source=${row.inventory_source}`);
        return `${row.id} host=${row.host_id}`;
      });

      if (row) {
        await step('controller-precheck-same-host', async () => {
          const r = await must('POST', `${P}/api/v1/vms/${row.id}/migrate/precheck`, { dest_host_id: row.host_id, live: true });
          if (!r.ok) throw new Error(JSON.stringify(r.checks || r).slice(0, 220));
          return `${(r.checks || []).length} checks`;
        });

        await step('controller-migrate-same-host', async () => {
          const before = (await details(B)).uuid;
          const t = await must('POST', `${P}/api/v1/vms/${row.id}/migrate`, { dest_host_id: row.host_id, live: true });
          await waitTask(t.task_id);
          const d = await details(B);
          if (d.state !== 'running' || d.uuid === before) throw new Error(`state=${d.state} uuid=${d.uuid}`);
          return `${before.slice(0, 8)} → ${d.uuid.slice(0, 8)}`;
        });

        await step('controller-ha-recreate', async () => {
          const before = (await details(B)).uuid;
          const t = await must('POST', `${P}/api/v1/vms/${row.id}/fluxvm/recover`, {});
          await waitTask(t.task_id);
          const d = await waitState(B, 'running', 90000).then(() => details(B));
          if (d.uuid === before) throw new Error('same instance');
          return `${before.slice(0, 8)} → ${d.uuid.slice(0, 8)}`;
        });

        await step('controller-recover-rejects-libvirt', async () => {
          const r = await api('POST', `${P}/api/v1/vms/00000000-0000-0000-0000-000000000000/fluxvm/recover`, {});
          if (ok(r.status)) throw new Error('accepted');
          return `${r.status}`;
        });
      }
    }
  }

  // --- cleanup -------------------------------------------------------------------
  for (const name of [A, B]) {
    await step(`delete-${name.slice(0, 5)}`, async () => {
      const r = await api('DELETE', q(name));
      if (!ok(r.status) && r.status !== 404) throw new Error(`${r.status} ${String(r.body).slice(0, 120)}`);
      await waitFor(`${name} gone`, async () => (await api('GET', q(name))).status === 404, 60000);
    });
  }
  try {
    onHost(`sudo rm -f ${SHARED_DISK} ${SHARED_DISK}.fluxvm-lock`);
  } catch {
    /* best effort */
  }

  log.append({ kind: 'SUMMARY', ok: fail === 0, note: `pass=${passed} fail=${fail} ${failed.join(',')}` });
  console.log(`FLUXVM_OPS_DONE pass=${passed} fail=${fail}${failed.length ? ` failed=${failed.join(',')}` : ''}`);
  process.exit(fail ? 1 : 0);
})().catch((e) => {
  console.error('FATAL', e);
  process.exit(1);
});

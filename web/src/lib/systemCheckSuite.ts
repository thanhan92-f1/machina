// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { getHealth } from '../api/node'
import { getPlatformInfo, type PlatformInfo } from '../api/system'
import {
  getHealthProblems,
  getHostVirtualization,
  getHostLibvirtBoot,
  getLibvirtSummary,
} from '../api/host'
import { listVMs } from '../api/vm'
import { getNodeInfo } from '../api/node'
import {
  getOpenStackStatus,
  postOpenStackTestConnection,
  listOpenStackFlavors,
  listOpenStackImages,
  listOpenStackNetworks,
  listOpenStackInstances,
  createOpenStackInstance,
  deleteOpenStackInstance,
} from '../api/openstack'
import { getK8sEnvironment, getK8sOverview } from '../api/k8s'
import { getHypersdkStatus } from '../api/hypersdk'
import { listServices } from '../api/extras'
import { formatUserError } from '../utils/apiError'

export type CheckStatus = 'pass' | 'warn' | 'fail' | 'skip'

export type CheckCategory =
  | 'api'
  | 'host'
  | 'libvirt'
  | 'openstack'
  | 'kubernetes'
  | 'hypersdk'
  | 'services'
  | 'websocket'

export const CHECK_CATEGORY_LABELS: Record<CheckCategory, string> = {
  api: 'API & platform',
  host: 'Host & virtualization',
  libvirt: 'Libvirt VMs',
  openstack: 'OpenStack',
  kubernetes: 'Kubernetes',
  hypersdk: 'HyperSDK',
  services: 'Systemd services',
  websocket: 'Live events (WebSocket)',
}

export interface CheckResult {
  id: string
  category: CheckCategory
  label: string
  status: CheckStatus
  message: string
  detail?: unknown
  durationMs: number
}

export interface SystemCheckContext {
  wsConnected: boolean
}

export interface SystemCheckProgress {
  category: CheckCategory
  label: string
}

export interface CheckSummary {
  pass: number
  warn: number
  fail: number
  skip: number
  overall: 'pass' | 'warn' | 'fail'
}

async function timedCheck(
  id: string,
  category: CheckCategory,
  label: string,
  fn: () => Promise<Pick<CheckResult, 'status' | 'message' | 'detail'>>,
): Promise<CheckResult> {
  const t0 = performance.now()
  try {
    const { status, message, detail } = await fn()
    return {
      id,
      category,
      label,
      status,
      message,
      detail,
      durationMs: Math.round(performance.now() - t0),
    }
  } catch (e: unknown) {
    return {
      id,
      category,
      label,
      status: 'fail',
      message: formatUserError(e),
      detail: e,
      durationMs: Math.round(performance.now() - t0),
    }
  }
}

function skip(
  id: string,
  category: CheckCategory,
  label: string,
  message: string,
): CheckResult {
  return { id, category, label, status: 'skip', message, durationMs: 0 }
}

async function runApiChecks(): Promise<CheckResult[]> {
  const out: CheckResult[] = []
  out.push(
    await timedCheck('api-health', 'api', 'API health endpoint', async () => {
      const h = await getHealth()
      if (!h.libvirt) {
        return { status: 'fail', message: 'libvirt not connected', detail: h }
      }
      return { status: 'pass', message: `healthy (${h.status})`, detail: h }
    }),
  )
  out.push(
    await timedCheck('api-platform-info', 'api', 'Platform info', async () => {
      const p = await getPlatformInfo()
      if (!p.version) {
        return { status: 'warn', message: 'version missing', detail: p }
      }
      return { status: 'pass', message: `Machina ${p.version}`, detail: p }
    }),
  )
  return out
}

async function runHostChecks(): Promise<CheckResult[]> {
  const out: CheckResult[] = []
  out.push(
    await timedCheck('host-problems', 'host', 'Host problem scan', async () => {
      const { items } = await getHealthProblems()
      const critical = items.filter((i) => i.severity === 'critical')
      const warnings = items.filter((i) => i.severity === 'warning')
      if (critical.length > 0) {
        return {
          status: 'fail',
          message: `${critical.length} critical, ${warnings.length} warning(s)`,
          detail: items,
        }
      }
      if (warnings.length > 0) {
        return {
          status: 'warn',
          message: `${warnings.length} warning(s)`,
          detail: items,
        }
      }
      return { status: 'pass', message: 'No host problems reported', detail: items }
    }),
  )
  out.push(
    await timedCheck('host-virt', 'host', 'CPU / KVM / libvirt sockets', async () => {
      const v = await getHostVirtualization()
      if (!v.cpu_virt_supported) {
        return { status: 'fail', message: v.hint || 'CPU virtualization not available', detail: v }
      }
      if (!v.kvm_device_present) {
        return { status: 'warn', message: '/dev/kvm missing or not accessible', detail: v }
      }
      if (!v.libvirt_system_socket_present && !v.libvirt_session_socket_present) {
        return { status: 'fail', message: 'No libvirt socket', detail: v }
      }
      return { status: 'pass', message: 'KVM and libvirt sockets OK', detail: v }
    }),
  )
  out.push(
    await timedCheck('host-libvirt-boot', 'host', 'Libvirt boot autostart', async () => {
      const b = await getHostLibvirtBoot()
      if (b.needs_attention) {
        return {
          status: 'warn',
          message: b.detail || 'libvirt may not start on boot',
          detail: b,
        }
      }
      return { status: 'pass', message: 'Boot autostart OK', detail: b }
    }),
  )
  out.push(
    await timedCheck('host-libvirt-summary', 'host', 'Libvirt connection summary', async () => {
      const s = await getLibvirtSummary()
      const connected =
        s.libvirt_connected ??
        s.primary_connected ??
        (s.qemu_system_connected || s.qemu_session_connected)
      if (!connected) {
        return {
          status: 'fail',
          message:
            'Daemon has no libvirt connection (check [libvirt] uri in config and machina-daemon logs)',
          detail: s,
        }
      }
      const parts: string[] = []
      if (s.primary_connected || (!s.dual_connection && connected)) parts.push('primary')
      if (s.qemu_system_connected) parts.push('system')
      if (s.qemu_session_connected) parts.push('session')
      const label = parts.length > 0 ? parts.join(', ') : 'connected'
      return {
        status: 'pass',
        message: `${label} · ${s.configured_uri}`,
        detail: s,
      }
    }),
  )
  return out
}

async function runLibvirtChecks(): Promise<CheckResult[]> {
  const out: CheckResult[] = []
  out.push(
    await timedCheck('libvirt-list-vms', 'libvirt', 'List VMs', async () => {
      const vms = await listVMs()
      return {
        status: 'pass',
        message: `${vms.length} VM(s) defined`,
        detail: { count: vms.length, names: vms.slice(0, 20).map((v) => v.name) },
      }
    }),
  )
  out.push(
    await timedCheck('libvirt-node-info', 'libvirt', 'Hypervisor node info', async () => {
      const n = await getNodeInfo()
      return {
        status: 'pass',
        message: `${n.hostname}: ${n.active_vms} running / ${n.defined_vms} defined`,
        detail: n,
      }
    }),
  )
  return out
}

async function runOpenStackChecks(platform: PlatformInfo): Promise<CheckResult[]> {
  const os = platform.openstack
  if (!os?.enabled) {
    return [
      skip('openstack-disabled', 'openstack', 'OpenStack integration', 'Disabled in daemon config'),
    ]
  }
  if (!os.configured) {
    return [
      skip(
        'openstack-not-configured',
        'openstack',
        'OpenStack integration',
        'Enabled but not wired (clouds.yaml / auth)',
      ),
    ]
  }

  const out: CheckResult[] = []
  out.push(
    await timedCheck('openstack-status', 'openstack', 'Connection status', async () => {
      const s = await getOpenStackStatus()
      if (!s.reachable) {
        return {
          status: 'fail',
          message: s.error || 'Keystone unreachable',
          detail: s,
        }
      }
      const parts = [
        s.compute_reachable ? 'Nova' : 'Nova off',
        s.glance_reachable ? 'Glance' : 'Glance off',
      ]
      if (!s.compute_reachable || !s.glance_reachable) {
        return {
          status: 'warn',
          message: `Keystone OK · ${parts.join(' · ')}`,
          detail: s,
        }
      }
      return {
        status: 'pass',
        message: `Keystone · ${parts.join(' · ')}`,
        detail: s,
      }
    }),
  )
  out.push(
    await timedCheck('openstack-test-connection', 'openstack', 'Test connection', async () => {
      const s = await postOpenStackTestConnection()
      if (!s.reachable) {
        return { status: 'fail', message: s.error || 'Test failed', detail: s }
      }
      if (!s.compute_reachable) {
        return {
          status: 'warn',
          message: 'Identity OK; Nova compute API unavailable (fake driver or service down)',
          detail: s,
        }
      }
      return { status: 'pass', message: 'Test connection OK', detail: s }
    }),
  )

  const catalogChecks = await Promise.all([
    timedCheck('openstack-flavors', 'openstack', 'Flavors catalog', async () => {
      const { flavors } = await listOpenStackFlavors()
      if (flavors.length === 0) {
        return { status: 'warn', message: 'No flavors', detail: flavors }
      }
      return { status: 'pass', message: `${flavors.length} flavor(s)`, detail: flavors }
    }),
    timedCheck('openstack-images', 'openstack', 'Images catalog', async () => {
      const { images } = await listOpenStackImages()
      const active = images.filter((i) => i.status?.toUpperCase() === 'ACTIVE')
      if (active.length === 0) {
        return { status: 'warn', message: 'No ACTIVE images', detail: images }
      }
      return { status: 'pass', message: `${active.length} ACTIVE image(s)`, detail: images }
    }),
    timedCheck('openstack-networks', 'openstack', 'Networks catalog', async () => {
      const { networks } = await listOpenStackNetworks()
      if (networks.length === 0) {
        return { status: 'warn', message: 'No networks', detail: networks }
      }
      return { status: 'pass', message: `${networks.length} network(s)`, detail: networks }
    }),
  ])
  out.push(...catalogChecks)
  return out
}

async function runKubernetesChecks(): Promise<CheckResult[]> {
  const out: CheckResult[] = []
  out.push(
    await timedCheck('k8s-environment', 'kubernetes', 'kubectl environment', async () => {
      const env = await getK8sEnvironment()
      if (!env.kubectl_on_path) {
        return { status: 'skip', message: 'kubectl not on PATH', detail: env }
      }
      if (!env.kubectl_server_reachable) {
        return {
          status: 'warn',
          message: env.kubeconfig_hint || 'API server not reachable',
          detail: env,
        }
      }
      return {
        status: 'pass',
        message: `API reachable · context ${env.current_context || 'default'}`,
        detail: env,
      }
    }),
  )
  out.push(
    await timedCheck('k8s-overview', 'kubernetes', 'Cluster overview', async () => {
      const env = await getK8sEnvironment()
      if (!env.kubectl_server_reachable) {
        return { status: 'skip', message: 'Skipped (API unreachable)', detail: env }
      }
      const o = await getK8sOverview()
      return {
        status: 'pass',
        message: `${o.ready_nodes}/${o.nodes} nodes ready · ${o.pods} pods`,
        detail: o,
      }
    }),
  )
  return out
}

async function runHypersdkChecks(platform: PlatformInfo): Promise<CheckResult[]> {
  if (!platform.hypersdk?.enabled) {
    return [skip('hypersdk-disabled', 'hypersdk', 'HyperSDK', 'Disabled in daemon config')]
  }
  return [
    await timedCheck('hypersdk-status', 'hypersdk', 'HyperSDK status', async () => {
      const s = await getHypersdkStatus()
      if (!s.reachable) {
        return {
          status: 'warn',
          message: s.last_error || `Unreachable at ${s.base_url}`,
          detail: s,
        }
      }
      return { status: 'pass', message: `Reachable · ${s.base_url}`, detail: s }
    }),
  ]
}

const WATCHED_SERVICES = [
  'machina-daemon.service',
  'libvirtd.service',
  'openstack-nova-compute.service',
  'neutron-server.service',
  'openstack-nova-api.service',
]

async function runServiceChecks(): Promise<CheckResult[]> {
  return [
    await timedCheck('services-key-units', 'services', 'Key systemd units', async () => {
      const all = await listServices()
      const watched = WATCHED_SERVICES.map((name) => {
        const svc = all.find((s) => s.name === name)
        return { name, svc, present: Boolean(svc) }
      }).filter((w) => w.present)

      const failed = watched.filter((w) => w.svc!.active_state !== 'active')
      const detail = watched.map((w) => ({
        name: w.name,
        active: w.svc!.active_state,
        enabled: w.svc!.enabled,
      }))

      if (failed.length > 0) {
        const names = failed.map((f) => `${f.name}=${f.svc!.active_state}`).join(', ')
        return {
          status: 'fail',
          message: `Not active: ${names}`,
          detail,
        }
      }
      if (watched.length === 0) {
        return { status: 'warn', message: 'No watched units found in service list', detail }
      }
      return {
        status: 'pass',
        message: `${watched.length} watched unit(s) active`,
        detail,
      }
    }),
  ]
}

function runWebSocketCheck(ctx: SystemCheckContext): CheckResult[] {
  return [
    {
      id: 'ws-connected',
      category: 'websocket',
      label: 'WebSocket to daemon',
      status: ctx.wsConnected ? 'pass' : 'warn',
      message: ctx.wsConnected
        ? 'Connected — live VM events enabled'
        : 'Not connected — UI may be stale until refresh',
      durationMs: 0,
    },
  ]
}

export function summarizeCheckResults(results: CheckResult[]): CheckSummary {
  const pass = results.filter((r) => r.status === 'pass').length
  const warn = results.filter((r) => r.status === 'warn').length
  const fail = results.filter((r) => r.status === 'fail').length
  const skip = results.filter((r) => r.status === 'skip').length
  const overall: CheckSummary['overall'] =
    fail > 0 ? 'fail' : warn > 0 ? 'warn' : 'pass'
  return { pass, warn, fail, skip, overall }
}

export function resultsByCategory(
  results: CheckResult[],
): Map<CheckCategory, CheckResult[]> {
  const map = new Map<CheckCategory, CheckResult[]>()
  for (const r of results) {
    const list = map.get(r.category) ?? []
    list.push(r)
    map.set(r.category, list)
  }
  return map
}

export const CHECK_CATEGORY_ORDER: CheckCategory[] = [
  'api',
  'host',
  'libvirt',
  'openstack',
  'kubernetes',
  'hypersdk',
  'services',
  'websocket',
]

export async function runSystemCheckSuite(
  ctx: SystemCheckContext,
  onProgress?: (p: SystemCheckProgress) => void,
): Promise<{ results: CheckResult[]; platform: PlatformInfo | null }> {
  const results: CheckResult[] = []
  let platform: PlatformInfo | null = null

  const runCat = async (
    category: CheckCategory,
    fn: () => Promise<CheckResult[]>,
  ) => {
    onProgress?.({ category, label: CHECK_CATEGORY_LABELS[category] })
    results.push(...(await fn()))
  }

  await runCat('api', async () => {
    const api = await runApiChecks()
    const pi = api.find((r) => r.id === 'api-platform-info')
    if (pi?.detail && typeof pi.detail === 'object') {
      platform = pi.detail as PlatformInfo
    } else {
      try {
        platform = await getPlatformInfo()
      } catch {
        platform = null
      }
    }
    return api
  })

  await runCat('host', runHostChecks)
  await runCat('libvirt', runLibvirtChecks)

  if (platform) {
    await runCat('openstack', () => runOpenStackChecks(platform!))
    await runCat('hypersdk', () => runHypersdkChecks(platform!))
  } else {
    results.push(
      skip('openstack-no-platform', 'openstack', 'OpenStack', 'Platform info unavailable'),
      skip('hypersdk-no-platform', 'hypersdk', 'HyperSDK', 'Platform info unavailable'),
    )
  }

  await runCat('kubernetes', runKubernetesChecks)
  await runCat('services', runServiceChecks)
  onProgress?.({ category: 'websocket', label: CHECK_CATEGORY_LABELS.websocket })
  results.push(...runWebSocketCheck(ctx))

  return { results, platform }
}

export function formatCheckReportMarkdown(
  results: CheckResult[],
  summary: CheckSummary,
): string {
  const lines: string[] = [
    '# Machina System Check',
    '',
    `Generated: ${new Date().toISOString()}`,
    '',
    `Summary: ${summary.pass} passed · ${summary.warn} warnings · ${summary.fail} failed · ${summary.skip} skipped`,
    '',
  ]
  const byCat = resultsByCategory(results)
  for (const cat of CHECK_CATEGORY_ORDER) {
    const rows = byCat.get(cat)
    if (!rows?.length) continue
    lines.push(`## ${CHECK_CATEGORY_LABELS[cat]}`)
    lines.push('')
    for (const r of rows) {
      const icon =
        r.status === 'pass' ? 'OK' : r.status === 'warn' ? 'WARN' : r.status === 'fail' ? 'FAIL' : 'SKIP'
      lines.push(`- [${icon}] **${r.label}**: ${r.message} (${r.durationMs}ms)`)
    }
    lines.push('')
  }
  return lines.join('\n')
}

/** Destructive OpenStack create → list → delete smoke (requires configured cloud + catalogs). */
export async function runOpenStackDeepSmoke(
  platform: PlatformInfo,
  onProgress?: (msg: string) => void,
): Promise<CheckResult[]> {
  const os = platform.openstack
  if (!os?.enabled || !os.configured) {
    return [
      skip('deep-openstack', 'openstack', 'OpenStack deep smoke', 'OpenStack not configured'),
    ]
  }

  const flavor = os.default_flavor || 'm1.tiny'
  const image = 'cirros-test'
  const network = os.default_network || 'private'
  const name = `syscheck-os-${Date.now()}`

  const results: CheckResult[] = []

  results.push(
    await timedCheck('deep-os-create', 'openstack', 'Create test instance', async () => {
      onProgress?.('Creating OpenStack instance…')
      const resp = await createOpenStackInstance({
        name,
        flavor,
        image,
        network,
        wait_until_active: true,
      })
      if (resp.status.toUpperCase() === 'ERROR') {
        return { status: 'fail', message: `Instance entered ERROR`, detail: resp }
      }
      return {
        status: 'pass',
        message: `Created ${resp.name} (${resp.status})`,
        detail: resp,
      }
    }),
  )

  const create = results[0]
  const instanceId =
    create.detail &&
    typeof create.detail === 'object' &&
    'id' in (create.detail as object)
      ? String((create.detail as { id: string }).id)
      : null

  if (create.status === 'fail' || !instanceId) {
    return results
  }

  results.push(
    await timedCheck('deep-os-list', 'openstack', 'List includes test instance', async () => {
      onProgress?.('Listing instances…')
      const { instances } = await listOpenStackInstances({ search: name })
      const found = instances.some((i) => i.id === instanceId || i.name === name)
      if (!found) {
        return { status: 'fail', message: 'Instance not found in list', detail: instances }
      }
      return { status: 'pass', message: 'Instance visible in Nova list', detail: instances }
    }),
  )

  results.push(
    await timedCheck('deep-os-delete', 'openstack', 'Delete test instance', async () => {
      onProgress?.('Deleting test instance…')
      await deleteOpenStackInstance(instanceId)
      return { status: 'pass', message: `Deleted ${name}`, detail: { id: instanceId } }
    }),
  )

  return results
}

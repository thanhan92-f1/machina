// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

// Native eBPF datapath on this host (daemon → local machina-bpfd). Types mirror
// bpf/machina-bpf/src/api.rs.

import { apiDelete, apiGet, apiGetBlob, apiPost, apiPut } from './client'

export type BpfMode = 'observe' | 'enforce'

export type BpfScope = { kind: 'host' } | { kind: 'vm'; name: string } | { kind: 'cgroup'; path: string }

export interface BpfPolicy {
  id: string
  name: string
  kind: string
  match: string
  enabled: boolean
  scope: BpfScope
  description: string
  created_at?: string | null
}

export interface BpfModeState {
  mode: BpfMode
  lease_expires_at?: string | null
  lease_remaining_secs?: number | null
  lease_expired?: boolean
}

export interface BpfIface {
  name: string
  ifindex: number
  vm?: string | null
  mac?: string | null
  scope: number
  flags: string[]
  guest_side: boolean
  xdp: boolean
  qos_egress_bps: number
  qos_ingress_bps: number
}

export interface BpfTelemetryConfig {
  exec: boolean
  fork: boolean
  connect: boolean
  flows: boolean
  dns: boolean
  l7?: boolean
  file_watch: string[]
  iface_patterns: string[]
}

export interface BpfStatus {
  available: boolean
  programs_compiled?: boolean
  version?: string
  features?: { kernel: string; btf: boolean; tcx: boolean; lsm_bpf: boolean; tracefs?: string | null; cgroup2: boolean }
  mode?: BpfModeState
  policies_total?: number
  policies_enabled?: number
  interfaces?: BpfIface[]
  cgroups?: string[]
  tracepoints?: string[]
  counters?: Record<string, number>
  telemetry?: BpfTelemetryConfig
  notes?: string[]
  /** Only when machina-bpfd is not reachable. */
  socket?: string
  error?: string
  probe?: Record<string, unknown>
}

export interface BpfFlowRecord {
  iface: string
  vm?: string | null
  proto: string
  local: string
  local_port: number
  remote: string
  remote_port: number
  origin: string
  tx_pkts: number
  tx_bytes: number
  rx_pkts: number
  rx_bytes: number
  first_seen: string
  last_seen: string
  verdict: string
}

export interface BpfNetEvent {
  ts: string
  kind: string
  verdict: string
  policy_id?: string | null
  iface?: string | null
  vm?: string | null
  proto: string
  local: string
  local_port: number
  remote: string
  remote_port: number
}

export interface BpfDnsRecord {
  ts: string
  vm?: string | null
  client: string
  server: string
  is_response: boolean
  qname: string
  qtype: string
  rcode: string
  answers: Array<{ name: string; rtype: string; ttl: number; data: string }>
}

export interface BpfL7Record {
  ts: string
  iface?: string | null
  vm?: string | null
  protocol: 'tls' | 'http' | 'ssh' | string
  direction: 'outbound' | 'inbound' | string
  client: string
  client_port: number
  server: string
  server_port: number
  host?: string | null
  alpn?: string[]
  tls_version?: string | null
  method?: string | null
  path?: string | null
  user_agent?: string | null
  banner?: string | null
}

export interface BpfAccountingRecord {
  vm?: string | null
  interfaces: string[]
  tx_bytes: number
  rx_bytes: number
  tx_pkts: number
  rx_pkts: number
  drops: number
  since: string
}

export interface BpfProcRecord {
  ts: string
  kind: string
  pid: number
  ppid?: number | null
  uid: number
  comm: string
  path?: string | null
  cmdline?: string | null
  unit?: string | null
  vm?: string | null
  container?: string | null
  denied: boolean
  killed: boolean
  policy_id?: string | null
  capability?: string | null
  daddr?: string | null
  dport?: number | null
}

export interface BpfAnomaly {
  id: string
  ts: string
  kind: string
  severity: string
  summary: string
  vm?: string | null
  remote?: string | null
}

export interface BpfNetHealth {
  drop_reasons: Array<{ reason: number; name: string; count: number }>
  tcp: Array<{ kind: string; addr: string; count: number }>
}

export interface BpfCapture {
  id: string
  iface: string
  vm?: string | null
  started_at: string
  ends_at: string
  packets: number
  bytes: number
  max_packets: number
  sample: number
  snaplen: number
  done: boolean
}

const B = '/api/v1/bpf'

const qs = (params: Record<string, string | number | undefined>) => {
  const p = new URLSearchParams()
  for (const [k, v] of Object.entries(params)) if (v !== undefined && v !== '') p.set(k, String(v))
  const s = p.toString()
  return s ? `?${s}` : ''
}

export const getBpfStatus = () => apiGet<BpfStatus>(`${B}/status`)
export const listBpfPolicies = () => apiGet<BpfPolicy[]>(`${B}/policies`)
export const applyBpfPolicy = (policy: Omit<BpfPolicy, 'created_at'>) => apiPost<BpfPolicy>(`${B}/policies`, policy)
export const removeBpfPolicy = (id: string) => apiDelete(`${B}/policies/${encodeURIComponent(id)}`)
export const setBpfMode = (mode: BpfMode, leaseSecs?: number) =>
  apiPut<BpfModeState>(`${B}/mode`, { mode, lease_secs: leaseSecs })

export const listBpfInterfaces = () => apiGet<BpfIface[]>(`${B}/interfaces`)
export const attachBpfInterface = (name: string, opts: { guest_side?: boolean; xdp?: boolean } = {}) =>
  apiPost<BpfIface[]>(`${B}/interfaces`, { name, ...opts })
export const detachBpfInterface = (name: string) => apiDelete(`${B}/interfaces/${encodeURIComponent(name)}`)

export const getBpfFlows = (limit = 200, vm?: string) => apiGet<BpfFlowRecord[]>(`${B}/flows${qs({ limit, vm })}`)
export const getBpfEvents = (limit = 200, kind?: string) => apiGet<BpfNetEvent[]>(`${B}/events${qs({ limit, kind })}`)
export const getBpfDns = (limit = 200) => apiGet<BpfDnsRecord[]>(`${B}/dns${qs({ limit })}`)
export const getBpfL7 = (limit = 200, protocol?: string, vm?: string) =>
  apiGet<BpfL7Record[]>(`${B}/l7${qs({ limit, protocol, vm })}`)
export const getBpfAccounting = (vm?: string) => apiGet<BpfAccountingRecord[]>(`${B}/accounting${qs({ vm })}`)
export const resetBpfAccounting = (vm?: string) => apiPost<{ reset: number }>(`${B}/accounting/reset`, vm ? { vm } : {})
export const getBpfProcesses = (limit = 200, kind?: string) =>
  apiGet<BpfProcRecord[]>(`${B}/processes${qs({ limit, kind })}`)
export const getBpfAnomalies = (limit = 100) => apiGet<BpfAnomaly[]>(`${B}/anomalies${qs({ limit })}`)
export const getBpfHealth = () => apiGet<BpfNetHealth>(`${B}/health`)

export const listBpfCaptures = () => apiGet<BpfCapture[]>(`${B}/captures`)
export const startBpfCapture = (body: {
  iface: string
  duration_secs?: number
  sample?: number
  snaplen?: number
  max_packets?: number
}) => apiPost<BpfCapture>(`${B}/captures`, body)
export const downloadBpfCapture = (id: string) => apiGetBlob(`${B}/captures/${encodeURIComponent(id)}`)

export const setBpfQos = (body: { iface?: string; vm?: string; egress_bps: number; ingress_bps: number }) =>
  apiPut<{ interfaces: string[]; egress_bps: number; ingress_bps: number }>(`${B}/qos`, body)

export const getBpfTelemetry = () => apiGet<BpfTelemetryConfig>(`${B}/telemetry`)
export const setBpfTelemetry = (t: BpfTelemetryConfig) => apiPut<BpfTelemetryConfig>(`${B}/telemetry`, t)

/** SSE URL for live events; topics: net, dns, l7, proc, anomaly (empty = all). */
export const bpfStreamUrl = (topics: string[] = []) => `${B}/stream${qs({ topics: topics.join(',') })}`

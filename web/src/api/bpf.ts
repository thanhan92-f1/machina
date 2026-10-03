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
  workload?: BpfWorkload | null
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
  workload?: BpfWorkload | null
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
  workload?: BpfWorkload | null
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
  workload?: BpfWorkload | null
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
  workload?: BpfWorkload | null
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

/** Owning workload attached to flow / event / TLS / SSL records. */
export interface BpfWorkload {
  kind: 'vm' | 'pod' | 'container' | 'service' | string
  ns?: string | null
  name: string
}

export interface BpfConnectHealth {
  addr: string
  port: number
  count: number
  failures: number
  avg_us: number
  max_us: number
  p90_le_us?: number | null
  hist: number[]
}

export interface BpfTcpPressure {
  addr: string
  srtt_us: number
  cwnd: number
  ssthresh: number
  mss: number
  total_retrans: number
  retrans_events: number
  delivery_rate_bps: number
  age_secs: number
}

export interface BpfNetHealth {
  drop_reasons: Array<{ reason: number; name: string; count: number }>
  tcp: Array<{ kind: string; addr: string; count: number }>
  connect?: BpfConnectHealth[]
  pressure?: BpfTcpPressure[]
  sockops?: string | null
}

export interface BpfIcmpError {
  iface: string
  vm?: string | null
  kind: string
  code: number
  family: string
  direction: string
  count: number
}

export interface BpfCniBackend {
  addr: string
  port: number
  remote?: boolean
  node?: string | null
}

export interface BpfCniService {
  addr: string
  port: number
  proto: number
  backends: BpfCniBackend[]
  name?: string | null
  affinity_secs?: number | null
  maglev: boolean
  affinity_entries: number
}

export interface BpfCniStatus {
  configured: boolean
  version: number
  node_addr?: string | null
  node_addr6?: string | null
  uplink?: string | null
  lb_mode: string
  xdp: boolean
  maglev_services: number
  endpoints: Array<{ ip: string; host_iface: string; pod?: string | null }>
  identities: number
  policy_entries: number
  services: number
  last_sync?: string | null
}

export interface BpfVmEdgeCounters {
  out_pkts: number
  out_bytes: number
  in_pkts: number
  in_bytes: number
  denied: number
  observed: number
  rate_dropped: number
}

export interface BpfVmEdgeStatus {
  vms: number
  rules: number
  groups: Record<string, number>
  taps: Array<{ vm: string; iface: string; identity: number; flags: string[]; stats: BpfVmEdgeCounters }>
  missing: string[]
  enforcing: boolean
}

export interface BpfSandboxConfig {
  mode: 'observe' | 'enforce' | string
  auto: boolean
  extra_devices: string[]
  egress_ports: string[]
}

export interface BpfSandboxHit {
  vm?: string | null
  cgroup?: string | null
  target: string
  count: number
}

export interface BpfSandboxStatus {
  config: BpfSandboxConfig
  devices: string[]
  attached: Record<string, string>
  enforcing: boolean
  device_hits: BpfSandboxHit[]
  egress_hits: BpfSandboxHit[]
  notes: string[]
}

export interface BpfShieldConfig {
  iface: string
  mode: 'off' | 'audit' | 'enforce'
  protect_all: boolean
  protected: string[]
  syn_pps: number
  udp_pps: number
  icmp_pps: number
  other_pps: number
  burst_secs: number
  allow: string[]
  deny: string[]
}

export interface BpfShieldStatus {
  config: BpfShieldConfig
  attached?: string | null
  enforcing: boolean
  stats: Record<
    | 'checked' | 'passed' | 'audited' | 'dropped' | 'dropped_bytes' | 'denied' | 'malformed'
    | 'syn_limited' | 'udp_limited' | 'icmp_limited' | 'other_limited',
    number
  >
  sources: Array<{ addr: string; class: string; hits: number }>
  tracked_sources: number
}

export interface BpfNodeIsoConfig {
  enabled: boolean
  iface: string
  lease_secs?: number | null
  dry_run: boolean
  allow_tcp: number[]
  allow_udp: number[]
  exempt: string[]
  allow_icmp: boolean
}

export interface BpfNodeIsoStatus {
  config: BpfNodeIsoConfig
  attached?: string | null
  isolating: boolean
  lease_expires_at?: string | null
  lease_remaining_secs?: number | null
  lease_expired: boolean
  stats: { checked: number; passed: number; dropped_in: number; dropped_out: number; would_drop: number }
}

export interface BpfTlsConfig {
  fingerprints: boolean
  fingerprint_rate: number
  ssl_uprobes: boolean
  ssl_comms: string[]
  ssl_all_processes: boolean
  ssl_rate: number
}

export interface BpfTlsStatus {
  config: BpfTlsConfig
  fingerprint_cgroup?: string | null
  ssl_libraries: string[]
  fingerprints_seen: number
  ssl_events_seen: number
  notes: string[]
}

export interface BpfTlsFingerprint {
  ts: string
  source: 'host' | 'tap' | string
  iface?: string | null
  cgroup?: string | null
  workload?: BpfWorkload | null
  client: string
  server: string
  server_port: number
  sni?: string | null
  alpn?: string[]
  tls_version: string
  ja3: string
  ja3_hash: string
  ja4: string
  truncated: boolean
}

export interface BpfSslRecord {
  ts: string
  workload?: BpfWorkload | null
  pid: number
  comm: string
  direction: string
  bytes: number
  protocol: string
  method?: string | null
  host?: string | null
  path?: string | null
  status?: number | null
}

export const workloadLabel = (w?: BpfWorkload | null) =>
  w ? `${w.kind} ${w.ns ? `${w.ns}/` : ''}${w.name}` : ''

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

export const getBpfIcmpErrors = () => apiGet<BpfIcmpError[]>(`${B}/icmp-errors`)
export const getBpfCni = () => apiGet<BpfCniStatus>(`${B}/cni`)
export const getBpfCniServices = () => apiGet<BpfCniService[]>(`${B}/cni/services`)
export const getBpfVmEdge = () => apiGet<BpfVmEdgeStatus>(`${B}/vm-edge`)
export const getBpfSandbox = () => apiGet<BpfSandboxStatus>(`${B}/vm-sandbox`)
export const setBpfSandbox = (c: BpfSandboxConfig) => apiPut<BpfSandboxStatus>(`${B}/vm-sandbox`, c)
export const getBpfShield = () => apiGet<BpfShieldStatus>(`${B}/shield`)
export const setBpfShield = (c: BpfShieldConfig) => apiPut<BpfShieldStatus>(`${B}/shield`, c)
export const getBpfNodeIso = () => apiGet<BpfNodeIsoStatus>(`${B}/node-iso`)
export const setBpfNodeIso = (c: BpfNodeIsoConfig) => apiPut<BpfNodeIsoStatus>(`${B}/node-iso`, c)
export const getBpfTls = () => apiGet<BpfTlsStatus>(`${B}/tls`)
export const setBpfTls = (c: BpfTlsConfig) => apiPut<BpfTlsStatus>(`${B}/tls`, c)
export const getBpfTlsFingerprints = (limit = 200) => apiGet<BpfTlsFingerprint[]>(`${B}/tls/fingerprints${qs({ limit })}`)
export const getBpfSslEvents = (limit = 200) => apiGet<BpfSslRecord[]>(`${B}/tls/ssl${qs({ limit })}`)

/** SSE URL for live events; topics: net, dns, l7, proc, anomaly (empty = all). */
export const bpfStreamUrl = (topics: string[] = []) => `${B}/stream${qs({ topics: topics.join(',') })}`

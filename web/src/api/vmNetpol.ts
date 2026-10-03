// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

// VM network policies (CiliumNetworkPolicy schema), VM labels and packet
// flows. `host` talks to this daemon; `fleet` to the controller, which pushes
// the compiled policy to every host's machina-bpfd.

import { apiDelete, apiGet, apiGetText, apiPost, apiPut, readJsonItemsList } from './client'
import { PLATFORM_CONTROLLER_PROXY } from './platform'

export type NetpolScope = 'host' | 'fleet'

export function netpolBase(scope: NetpolScope): string {
  return scope === 'fleet' ? `${PLATFORM_CONTROLLER_PROXY}/api/v1` : '/api/v1'
}

export interface NetpolIssue {
  path: string
  message: string
}

export interface VmNetworkPolicy {
  name: string
  kind: string
  description?: string | null
  labels: Record<string, string>
  annotations: Record<string, string>
  specs: unknown[]
  yaml: string
  selected_vms: string[]
  /** Fleet only. */
  enabled?: boolean
  generation?: number
  updated_at?: string
}

export interface NetpolEndpoint {
  name: string
  host?: string | null
  identity: number
  labels: Record<string, string>
  addresses: string[]
  ingress_enforced: boolean
  egress_enforced: boolean
  policies: string[]
}

export interface NetpolSelector {
  policy: string
  path: string
  selector: string
  vms: string[]
}

export interface NetpolPreview {
  valid: boolean
  errors: NetpolIssue[]
  warnings: NetpolIssue[]
  compile_warnings: string[]
  policies: VmNetworkPolicy[]
  rules: number
  endpoints: NetpolEndpoint[]
  selectors: NetpolSelector[]
}

export interface NetpolSync {
  at?: string | null
  ok: boolean
  error?: string | null
  skipped?: string | null
  vms: number
  rules: number
  peers: number
  warnings: string[]
}

export interface NetpolApplyResult {
  applied: string[]
  warnings: NetpolIssue[]
  sync?: NetpolSync | NetpolSync[] | Record<string, unknown>
}

export interface TraceEndpoint {
  input: string
  vm?: string | null
  identity: number
  kind: string
}

export interface TraceSide {
  direction: string
  vm?: string | null
  enforced: boolean
  verdict: 'allowed' | 'denied' | 'default-deny' | 'no-policy' | 'not-a-vm' | string
  rule?: string | null
}

export interface TraceResult {
  allowed: boolean
  from: TraceEndpoint
  to: TraceEndpoint
  protocol: string
  port: number
  egress: TraceSide
  ingress: TraceSide
  summary: string
}

export interface TraceQuery {
  from: string
  to: string
  protocol?: string
  port?: number
  icmp_type?: number
}

export interface NetpolHostStatus {
  host_id: string
  hostname: string
  synced_at?: string | null
  ok?: boolean
  error?: string | null
  vms?: number
  rules?: number
  peers?: number
  reachable?: boolean
  owner?: string | null
  enforcing?: boolean
  taps?: number
  missing?: string[]
  cilium?: string | null
}

export interface NetpolStatus {
  policies: number
  managed_by: 'local' | 'controller' | string
  /** Daemon only. */
  last_sync?: NetpolSync | null
  edge?: { owner: string; enforcing: boolean; flow_log: boolean; taps: unknown[]; peers?: number } | null
  enforcement?: { mode?: string; lease_remaining_secs?: number | null } | null
  bpfd_available?: boolean
  cilium?: string | null
  /** Controller only. */
  hosts?: NetpolHostStatus[]
}

export interface VmFlowRecord {
  ts: string
  host?: string | null
  iface: string
  vm: string
  direction: 'ingress' | 'egress' | string
  src: string
  src_port: number
  dst: string
  dst_port: number
  src_vm?: string | null
  dst_vm?: string | null
  src_labels: Record<string, string>
  dst_labels: Record<string, string>
  src_identity: number
  dst_identity: number
  proto: string
  tcp_flags: string
  icmp_type?: number | null
  bytes: number
  verdict: 'FORWARDED' | 'DROPPED' | 'AUDIT' | string
  drop_reason?: string | null
  policy?: string | null
}

export interface FlowFilter {
  vm?: string
  from_vm?: string
  to_vm?: string
  label?: string
  ip?: string
  cidr?: string
  port?: string
  protocol?: string
  verdict?: string
  drop_reason?: string
  policy?: string
  direction?: string
  host?: string
}

export function flowQuery(f: FlowFilter, extra: Record<string, string | number> = {}): string {
  const q = new URLSearchParams()
  for (const [k, v] of Object.entries({ ...f, ...extra })) {
    if (v !== undefined && v !== null && String(v).trim() !== '') q.set(k, String(v).trim())
  }
  const s = q.toString()
  return s ? `?${s}` : ''
}

const P = '/vm-network-policies'

export async function listVmNetpols(scope: NetpolScope): Promise<{ items: VmNetworkPolicy[]; warnings: string[] }> {
  const r = await apiGet<{ items?: VmNetworkPolicy[]; warnings?: string[] }>(`${netpolBase(scope)}${P}`)
  return { items: r.items ?? [], warnings: r.warnings ?? [] }
}

export const getVmNetpolYaml = (scope: NetpolScope, name: string) =>
  apiGetText(`${netpolBase(scope)}${P}/${encodeURIComponent(name)}?format=yaml`)

export const validateVmNetpol = (scope: NetpolScope, yaml: string) =>
  apiPost<NetpolPreview>(`${netpolBase(scope)}${P}/validate`, { yaml })

export const applyVmNetpol = (scope: NetpolScope, yaml: string) =>
  apiPost<NetpolApplyResult>(`${netpolBase(scope)}${P}`, { yaml })

export const deleteVmNetpol = (scope: NetpolScope, name: string) =>
  apiDelete(`${netpolBase(scope)}${P}/${encodeURIComponent(name)}`)

export const setVmNetpolEnabled = (name: string, enabled: boolean) =>
  apiPut<{ enabled: boolean }>(`${netpolBase('fleet')}${P}/${encodeURIComponent(name)}/enabled`, { enabled })

export const traceVmNetpol = (scope: NetpolScope, q: TraceQuery) =>
  apiPost<TraceResult>(`${netpolBase(scope)}${P}/trace`, q)

export const listNetpolEndpoints = async (scope: NetpolScope) =>
  (await readJsonItemsList<NetpolEndpoint>(`${netpolBase(scope)}${P}/endpoints`)).items

export const listNetpolSelectors = async (scope: NetpolScope) =>
  (await readJsonItemsList<NetpolSelector>(`${netpolBase(scope)}${P}/selectors`)).items

export const getNetpolStatus = (scope: NetpolScope) => apiGet<NetpolStatus>(`${netpolBase(scope)}${P}/status`)

export const syncFleetNetpol = () => apiPost<unknown>(`${netpolBase('fleet')}${P}/sync`)

export const listFlows = async (scope: NetpolScope, f: FlowFilter, limit = 500) =>
  (await readJsonItemsList<VmFlowRecord>(`${netpolBase(scope)}/flows${flowQuery(f, { limit })}`)).items

export const flowStreamUrl = (scope: NetpolScope, f: FlowFilter, last = 100) =>
  `${netpolBase(scope)}/flows/stream${flowQuery(f, { last })}`

/** Daemon: VM name. Controller: VM id. */
export async function getVmLabels(scope: NetpolScope, vm: string): Promise<Record<string, string>> {
  const r = await apiGet<{ labels?: Record<string, string> }>(`${netpolBase(scope)}/vms/${encodeURIComponent(vm)}/labels`)
  return r.labels ?? {}
}

export async function setVmLabels(scope: NetpolScope, vm: string, labels: Record<string, string>): Promise<Record<string, string>> {
  const r = await apiPut<{ labels?: Record<string, string> }>(`${netpolBase(scope)}/vms/${encodeURIComponent(vm)}/labels`, { labels })
  return r.labels ?? labels
}

/** Kubernetes label syntax, same rules the servers enforce. */
export function labelError(key: string, value: string): string | null {
  const name = /^[A-Za-z0-9]([-A-Za-z0-9_.]{0,61}[A-Za-z0-9])?$/
  const parts = key.split('/')
  if (parts.length > 2 || !key) return 'invalid key'
  const [prefix, n] = parts.length === 2 ? parts : [null, parts[0]]
  if (prefix !== null && (!prefix || prefix.length > 253 || !/^[a-z0-9]([-a-z0-9.]*[a-z0-9])?$/.test(prefix))) return 'invalid key prefix'
  if (!name.test(n)) return 'invalid key name'
  if (value && !name.test(value)) return 'invalid value'
  return null
}

export const NETPOL_TEMPLATES: Array<{ id: string; label: string; yaml: string }> = [
  {
    id: 'web-db',
    label: 'Web → DB on 5432',
    yaml: `apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: db-from-web
spec:
  description: Only web VMs reach the database, on 5432/TCP
  endpointSelector:
    matchLabels:
      app: db
  ingress:
    - fromEndpoints:
        - matchLabels:
            app: web
      toPorts:
        - ports:
            - port: "5432"
              protocol: TCP
`,
  },
  {
    id: 'default-deny',
    label: 'Default deny (ingress + egress)',
    yaml: `apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: default-deny
spec:
  description: Isolate every VM labelled env=prod; add allow policies on top
  endpointSelector:
    matchLabels:
      env: prod
  ingress:
    - {}
  egress:
    - {}
`,
  },
  {
    id: 'egress-dns-https',
    label: 'Egress: DNS + HTTPS to world',
    yaml: `apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: egress-dns-https
spec:
  endpointSelector:
    matchLabels:
      tier: frontend
  egress:
    - toEntities:
        - world
      toPorts:
        - ports:
            - port: "53"
              protocol: UDP
            - port: "443"
              protocol: TCP
`,
  },
  {
    id: 'deny-metadata',
    label: 'Deny a CIDR (egressDeny)',
    yaml: `apiVersion: cilium.io/v2
kind: CiliumClusterwideNetworkPolicy
metadata:
  name: deny-metadata
spec:
  description: No VM may reach the link-local metadata range or 10.66.0.0/16
  endpointSelector: {}
  egressDeny:
    - toCIDRSet:
        - cidr: 169.254.0.0/16
        - cidr: 10.66.0.0/16
`,
  },
  {
    id: 'icmp',
    label: 'Allow ping from the host',
    yaml: `apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: ping-from-host
spec:
  endpointSelector: {}
  ingress:
    - fromEntities:
        - host
      icmps:
        - fields:
            - type: EchoRequest
              family: IPv4
`,
  },
  {
    id: 'ssh-range',
    label: 'SSH from a CIDR, except one subnet',
    yaml: `apiVersion: cilium.io/v2
kind: CiliumNetworkPolicy
metadata:
  name: ssh-from-office
spec:
  endpointSelector:
    matchExpressions:
      - key: role
        operator: In
        values: [bastion, admin]
  ingress:
    - fromCIDRSet:
        - cidr: 192.168.0.0/16
          except:
            - 192.168.99.0/24
      toPorts:
        - ports:
            - port: "22"
              protocol: TCP
`,
  },
]

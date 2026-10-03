// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

// Read-only, human summary of one CiliumNetworkPolicy spec.

type Obj = Record<string, unknown>

export interface RuleSummary {
  direction: 'ingress' | 'egress'
  deny: boolean
  /** `{}` — selects the VM (default deny) but allows nothing. */
  allowNothing: boolean
  text: string
}

export interface SpecSummary {
  subject: string
  description?: string
  rules: RuleSummary[]
  defaultDeny: string[]
}

const isObj = (v: unknown): v is Obj => typeof v === 'object' && v !== null && !Array.isArray(v)
const arr = (v: unknown): unknown[] => (Array.isArray(v) ? v : [])

export function selectorText(sel: unknown): string {
  if (!isObj(sel)) return 'all VMs'
  const parts: string[] = []
  if (isObj(sel.matchLabels)) {
    for (const [k, v] of Object.entries(sel.matchLabels)) parts.push(`${k.replace(/^(k8s|any|machina):/, '')}=${String(v)}`)
  }
  for (const e of arr(sel.matchExpressions)) {
    if (!isObj(e)) continue
    const key = String(e.key ?? '').replace(/^(k8s|any|machina):/, '')
    const op = String(e.operator ?? '')
    const vals = arr(e.values).map(String)
    if (op === 'Exists') parts.push(`has ${key}`)
    else if (op === 'DoesNotExist') parts.push(`no ${key}`)
    else parts.push(`${key} ${op === 'NotIn' ? 'not in' : 'in'} (${vals.join(', ')})`)
  }
  return parts.length ? parts.join(', ') : 'all VMs'
}

function portsText(rule: Obj): string {
  const out: string[] = []
  for (const tp of arr(rule.toPorts)) {
    if (!isObj(tp)) continue
    for (const p of arr(tp.ports)) {
      if (!isObj(p)) continue
      const proto = String(p.protocol ?? 'ANY').toUpperCase()
      const port = String(p.port ?? '0')
      const end = p.endPort ? `-${String(p.endPort)}` : ''
      out.push(port === '0' ? `any ${proto} port` : `${port}${end}/${proto}`)
    }
    if (isObj(tp.rules)) out.push(`L7 ${Object.keys(tp.rules).join('/')} rules (not enforced yet)`)
  }
  for (const ic of arr(rule.icmps)) {
    if (!isObj(ic)) continue
    for (const f of arr(ic.fields)) {
      if (isObj(f)) out.push(`ICMP${f.family === 'IPv6' ? 'v6' : ''} ${String(f.type ?? '')}`)
    }
  }
  return out.join(', ')
}

function peersText(rule: Obj, dir: 'from' | 'to'): string[] {
  const out: string[] = []
  for (const s of arr(rule[`${dir}Endpoints`])) out.push(`VMs ${selectorText(s)}`)
  for (const c of arr(rule[`${dir}CIDR`])) out.push(String(c))
  for (const c of arr(rule[`${dir}CIDRSet`])) {
    if (!isObj(c)) continue
    const ex = arr(c.except).map(String)
    out.push(`${String(c.cidr ?? c.cidrGroupRef ?? '?')}${ex.length ? ` except ${ex.join(', ')}` : ''}`)
  }
  for (const e of arr(rule[`${dir}Entities`])) out.push(`entity:${String(e)}`)
  for (const f of arr(rule.toFQDNs)) {
    if (dir === 'to' && isObj(f)) out.push(`fqdn:${String(f.matchName ?? f.matchPattern ?? '?')} (not enforced yet)`)
  }
  if (dir === 'to' && arr(rule.toServices).length) out.push('services (not enforced yet)')
  if (dir === 'to' && arr(rule.toGroups).length) out.push('groups (not enforced yet)')
  return out
}

export function summarizeSpec(spec: unknown): SpecSummary {
  const s = isObj(spec) ? spec : {}
  const subject = isObj(s.nodeSelector) ? `hosts ${selectorText(s.nodeSelector)}` : selectorText(s.endpointSelector)
  const rules: RuleSummary[] = []
  const dd = isObj(s.enableDefaultDeny) ? s.enableDefaultDeny : {}
  const defaultDeny: string[] = []
  for (const [key, direction, deny] of [
    ['ingress', 'ingress', false],
    ['ingressDeny', 'ingress', true],
    ['egress', 'egress', false],
    ['egressDeny', 'egress', true],
  ] as const) {
    for (const r of arr(s[key])) {
      if (!isObj(r)) continue
      const dirWord = direction === 'ingress' ? 'from' : 'to'
      const peers = peersText(r, dirWord)
      const ports = portsText(r)
      const requires = arr(r[`${dirWord}Requires`]).map((x) => selectorText(x))
      const empty = peers.length === 0 && !ports
      let text: string
      if (empty) text = deny ? 'nothing' : 'nothing (selects the VM so everything else is denied)'
      else text = `${dirWord} ${peers.length ? peers.join('; ') : 'any peer'}${ports ? ` on ${ports}` : ''}`
      if (requires.length) text += ` — peer must also match ${requires.join(' and ')}`
      rules.push({ direction, deny, allowNothing: empty && !deny, text })
    }
  }
  for (const d of ['ingress', 'egress'] as const) {
    const has = arr(s[d]).length > 0 || arr(s[`${d}Deny`]).length > 0
    if (has && dd[d] !== false) defaultDeny.push(d)
  }
  return { subject, description: typeof s.description === 'string' ? s.description : undefined, rules, defaultDeny }
}

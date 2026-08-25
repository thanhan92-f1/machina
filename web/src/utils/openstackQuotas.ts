// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

export interface QuotaRow {
  label: string
  used: number
  max: number
  /** API field for PUT quota updates */
  key?: string
  service?: 'compute' | 'cinder' | 'neutron'
}

const NOVA_LIMIT_KEYS: Record<string, string> = {
  Instances: 'instances',
  Cores: 'cores',
  RAMSize: 'ram',
  FloatingIps: 'floating_ips',
  SecurityGroups: 'security_groups',
  SecurityGroupsRules: 'security_group_rules',
  KeyPairs: 'key_pairs',
  ServerGroups: 'server_groups',
  ServerMeta: 'metadata_items',
  ImageMeta: 'metadata_items',
}

const CINDER_LIMIT_KEYS: Record<string, string> = {
  VolumeGigabytes: 'gigabytes',
  Volumes: 'volumes',
  Snapshots: 'snapshots',
  Backups: 'backups',
  BackupGigabytes: 'backup_gigabytes',
}

function limitKeyFromMaxKey(maxKey: string, service: 'compute' | 'cinder'): string | undefined {
  const m = maxKey.match(/^maxTotal(.+)$/i)
  if (!m) return undefined
  const stem = m[1]
  const map = service === 'cinder' ? CINDER_LIMIT_KEYS : NOVA_LIMIT_KEYS
  if (map[stem]) return map[stem]
  return stem.replace(/([A-Z])/g, '_$1').toLowerCase().replace(/^_/, '')
}

/** Extract used/max rows from Nova or Cinder limits JSON. */
export function parseQuotaRows(limits: unknown, service: 'compute' | 'cinder'): QuotaRow[] {
  if (!limits || typeof limits !== 'object') return []
  const root = limits as Record<string, unknown>
  const bag =
    root.absolute && typeof root.absolute === 'object'
      ? (root.absolute as Record<string, unknown>)
      : root

  const rows: QuotaRow[] = []
  const seen = new Set<string>()

  for (const [key, val] of Object.entries(bag)) {
    if (typeof val !== 'number') continue
    const usedMatch = key.match(/^total(.+)Used$/i)
    if (usedMatch) {
      const stem = usedMatch[1]
      const maxKey = Object.keys(bag).find(
        (k) => k.toLowerCase() === `maxtotal${stem.toLowerCase()}`,
      )
      const maxVal = maxKey && typeof bag[maxKey] === 'number' ? (bag[maxKey] as number) : -1
      const label = stem.replace(/([A-Z])/g, ' $1').trim()
      rows.push({
        label,
        used: val,
        max: maxVal,
        key: maxKey ? limitKeyFromMaxKey(maxKey, service) : undefined,
        service,
      })
      seen.add(stem.toLowerCase())
      continue
    }
    const maxMatch = key.match(/^maxTotal(.+)$/i)
    if (maxMatch && !seen.has(maxMatch[1].toLowerCase())) {
      const stem = maxMatch[1]
      const usedKey = Object.keys(bag).find(
        (k) => k.toLowerCase() === `total${stem.toLowerCase()}used`,
      )
      if (!usedKey) {
        rows.push({
          label: stem.replace(/([A-Z])/g, ' $1').trim(),
          used: 0,
          max: val,
          key: limitKeyFromMaxKey(key, service),
          service,
        })
      }
    }
  }

  rows.sort((a, b) => a.label.localeCompare(b.label))
  return rows
}

/** Extract used/limit rows from Neutron quota details JSON. */
export function parseNeutronQuotaRows(quotas: unknown): QuotaRow[] {
  if (!quotas || typeof quotas !== 'object') return []
  const root = quotas as Record<string, unknown>
  const bag =
    root.quota && typeof root.quota === 'object'
      ? (root.quota as Record<string, unknown>)
      : root

  const rows: QuotaRow[] = []
  for (const [key, val] of Object.entries(bag)) {
    if (val && typeof val === 'object' && 'used' in val && 'limit' in val) {
      const d = val as { used: number; limit: number }
      rows.push({
        label: key.replace(/_/g, ' '),
        used: d.used,
        max: d.limit,
        key,
        service: 'neutron',
      })
    } else if (typeof val === 'number') {
      rows.push({
        label: key.replace(/_/g, ' '),
        used: 0,
        max: val,
        key,
        service: 'neutron',
      })
    }
  }
  rows.sort((a, b) => a.label.localeCompare(b.label))
  return rows
}

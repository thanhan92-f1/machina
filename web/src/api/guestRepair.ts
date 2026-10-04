// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { apiPost } from './client'

/** Result of an offline GuestKit diagnose/repair on a powered-off VM's disk (daemon on the VM's host). */
export type GuestRepairReport = {
  vm: string
  disk: string
  action: 'diagnose' | 'repair'
  dry_run: boolean
  backup: boolean
  /** GuestKit output; `diagnose` asks for JSON. */
  output: string
  exit_ok: boolean
}

const base = (name: string) => `/api/v1/vms/${encodeURIComponent(name)}/guest-repair`

/** True when this daemon knows the route (GET on a POST-only route answers 405; older daemons answer 404). */
export const bootDoctorSupported = async (name: string): Promise<boolean> => {
  try {
    const res = await fetch(`${base(name)}/diagnose`, { credentials: 'same-origin' })
    return res.status === 405
  } catch {
    return false
  }
}

export const diagnoseGuestDisk = (name: string) => apiPost<GuestRepairReport>(`${base(name)}/diagnose`, {})

export const repairGuestDisk = (name: string, opts: { dryRun: boolean; backup?: boolean }) =>
  apiPost<GuestRepairReport>(`${base(name)}/apply`, { dry_run: opts.dryRun, backup: opts.backup ?? true })

/** Pull a score and readable findings out of GuestKit's JSON; fall back to the raw text. */
export function summariseDoctorOutput(output: string): { score: number | null; findings: string[]; raw: string } {
  let data: unknown = null
  try {
    data = JSON.parse(output)
  } catch {
    // GuestKit may print text before/after the JSON — try the outermost braces.
    const a = output.indexOf('{')
    const b = output.lastIndexOf('}')
    if (a >= 0 && b > a) {
      try { data = JSON.parse(output.slice(a, b + 1)) } catch { data = null }
    }
  }
  if (!data || typeof data !== 'object') return { score: null, findings: [], raw: output }
  const obj = data as Record<string, unknown>
  const scoreKey = ['boot_score', 'score', 'bootScore'].find((k) => typeof obj[k] === 'number')
  const score = scoreKey ? Math.max(0, Math.min(100, Math.round(obj[scoreKey] as number))) : null
  const findings: string[] = []
  const take = (v: unknown) => {
    if (typeof v === 'string') findings.push(v)
    else if (v && typeof v === 'object') {
      const o = v as Record<string, unknown>
      const text = [o.title, o.message, o.description, o.summary, o.detail].find((x) => typeof x === 'string')
      if (typeof text === 'string') findings.push(text)
    }
  }
  for (const key of ['blockers', 'warnings', 'findings', 'issues', 'root_causes', 'causes', 'recommendations']) {
    const v = obj[key]
    if (Array.isArray(v)) v.forEach(take)
  }
  return { score, findings: [...new Set(findings)].slice(0, 12), raw: output }
}

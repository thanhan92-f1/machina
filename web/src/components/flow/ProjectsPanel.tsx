// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

// Fleet Cloud project networking: default isolation between projects,
// per-project egress allowlists and egress IPs. Settings become generated
// policies (and SNAT rules on the hosts) on the controller.

import { Fragment, useCallback, useEffect, useState } from 'react'
import { MacGlassPanel } from '../platform/mac/PlatformMacUi'
import { TahoeTableWrap } from '../platform/tahoe/TahoeListKit'
import { Empty, Field, headRowCls, rowCls, thCls } from '../bpf/shared'
import {
  describeEgress,
  listEgressIps,
  listProjects,
  resetProject,
  setProject,
  splitPorts,
  type EgressHostStatus,
  type ProjectIsolation,
  type ProjectNet,
  type ProjectRow,
} from '../../api/vmNetpol'
import { formatUserError } from '../../utils/apiError'
import { useToastContext } from '../../contexts/ToastContext'
import { statusPillClasses, statusToneClass } from '../../utils/semanticColors'

function body(s: ProjectNet): Omit<ProjectNet, 'project'> {
  return {
    isolation: s.isolation,
    allow_host: s.allow_host,
    egress_restricted: s.egress_restricted,
    egress_allow: s.egress_allow,
    egress_ips: s.egress_ips,
  }
}

export default function ProjectsPanel({ onChanged }: { onChanged: () => void }) {
  const toast = useToastContext()
  const [def, setDef] = useState<ProjectNet | null>(null)
  const [rows, setRows] = useState<ProjectRow[]>([])
  const [hosts, setHosts] = useState<EgressHostStatus[]>([])
  const [open, setOpen] = useState<string | null>(null)
  const [dest, setDest] = useState({ to: '', ports: '' })
  const [eip, setEip] = useState({ host: '', ip: '' })
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const load = useCallback(async () => {
    try {
      const r = await listProjects()
      setDef(r.default)
      setRows(r.items)
      setError(null)
    } catch (e: unknown) {
      setError(formatUserError(e))
    }
    try {
      setHosts((await listEgressIps()).items)
    } catch {
      setHosts([])
    }
  }, [])

  useEffect(() => { void load() }, [load])

  const save = async (project: string, s: ProjectNet, msg: string) => {
    setBusy(true)
    try {
      await setProject(project, body(s))
      toast.success(msg)
      await load()
      onChanged()
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    } finally {
      setBusy(false)
    }
  }

  const reset = async (project: string) => {
    if (!window.confirm(`Drop every network setting of project ${project}? It follows the default again.`)) return
    try {
      await resetProject(project)
      toast.success(`Project ${project} follows the default`)
      await load()
      onChanged()
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    }
  }

  if (error) return <MacGlassPanel title="Projects"><div className={statusToneClass('error')}>{error}</div></MacGlassPanel>
  if (!def) return <MacGlassPanel title="Projects"><Empty>Loading…</Empty></MacGlassPanel>

  const hostNames = hosts.map((h) => h.hostname)

  return (
    <div className="space-y-4">
      <MacGlassPanel
        title="Default project isolation"
        subtitle="An isolated project's VMs accept connections only from VMs of the same project (and the host, if allowed). Projects follow this default unless set below. Ordinary allow policies still open exceptions; drops need the enforcement lease."
      >
        <div className="flex flex-wrap items-center gap-3">
          <select
            aria-label="Default isolation"
            className="input text-sm"
            value={def.isolation}
            disabled={busy}
            onChange={(e) => void save('*', { ...def, isolation: e.target.value as ProjectIsolation }, `Projects are ${e.target.value} by default`)}
          >
            <option value="open">Open by default</option>
            <option value="isolated">Isolated by default</option>
          </select>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              aria-label="Default allows the host"
              checked={def.allow_host}
              disabled={busy || def.isolation !== 'isolated'}
              onChange={(e) => void save('*', { ...def, allow_host: e.target.checked }, e.target.checked ? 'The host can reach isolated projects' : 'The host is blocked from isolated projects')}
            />
            Host may connect
          </label>
        </div>
      </MacGlassPanel>

      <MacGlassPanel title="Projects" subtitle="Isolation, egress allowlist and egress IPs per Fleet Cloud project.">
        {rows.length === 0 ? (
          <Empty>No projects yet. VMs get a project in Fleet Cloud.</Empty>
        ) : (
          <TahoeTableWrap>
            <table className="w-full text-xs" aria-label="Project networking">
              <thead>
                <tr className={headRowCls}>
                  <th scope="col" className={thCls}>Project</th>
                  <th scope="col" className={thCls}>VMs</th>
                  <th scope="col" className={thCls}>Isolation</th>
                  <th scope="col" className={thCls}>Egress</th>
                  <th scope="col" className={thCls}>Egress IPs</th>
                  <th scope="col" className="py-2"><span className="sr-only">Actions</span></th>
                </tr>
              </thead>
              <tbody>
                {rows.map((r) => {
                  const s = r.settings
                  const ips = Object.entries(s.egress_ips)
                  return (
                    <Fragment key={r.project}>
                      <tr className={rowCls}>
                        <td className="py-2 pr-2 font-medium">{r.project}</td>
                        <td className="py-2 pr-2" title={r.vms.join(', ')}>{r.vms.length}</td>
                        <td className="py-2 pr-2">
                          <select
                            aria-label={`Isolation of ${r.project}`}
                            className="input text-xs py-1"
                            value={s.isolation}
                            disabled={busy}
                            onChange={(e) => void save(r.project, { ...s, isolation: e.target.value as ProjectIsolation }, `Project ${r.project}: ${e.target.value}`)}
                          >
                            <option value="inherit">Default ({def.isolation})</option>
                            <option value="isolated">Isolated</option>
                            <option value="open">Open</option>
                          </select>
                          <span className={`ml-2 ${statusPillClasses(r.isolated ? 'ok' : 'neutral')}`}>{r.isolated ? (r.allow_host ? 'Isolated' : 'Isolated, no host') : 'Open'}</span>
                        </td>
                        <td className="py-2 pr-2 max-w-[280px] truncate" title={describeEgress(s)}>{describeEgress(s)}</td>
                        <td className="py-2 pr-2 font-mono">{ips.length ? ips.map(([h, ip]) => `${ip} @ ${h}`).join(', ') : '—'}</td>
                        <td className="py-2 text-right whitespace-nowrap">
                          <button type="button" className="btn-secondary text-xs mr-2" onClick={() => setOpen(open === r.project ? null : r.project)}>
                            {open === r.project ? 'Close' : 'Egress…'}
                          </button>
                          {r.explicit && <button type="button" className="btn-secondary text-xs" onClick={() => void reset(r.project)}>Reset</button>}
                        </td>
                      </tr>
                      {open === r.project && (
                        <tr className={rowCls}>
                          <td colSpan={6} className="py-3">
                            <div className="space-y-3" aria-label={`Egress of ${r.project}`}>
                              <label className="flex items-center gap-2 text-sm">
                                <input
                                  type="checkbox"
                                  aria-label={`Limit egress of ${r.project}`}
                                  checked={s.egress_restricted}
                                  disabled={busy}
                                  onChange={(e) => void save(r.project, { ...s, egress_restricted: e.target.checked }, e.target.checked ? `Project ${r.project}: egress limited to the allowlist` : `Project ${r.project}: egress unrestricted`)}
                                />
                                Limit egress to the allowlist (plus the project's VMs, the host and DNS)
                              </label>
                              {s.egress_allow.length > 0 && (
                                <ul className="space-y-1">
                                  {s.egress_allow.map((e) => (
                                    <li key={e.to} className="flex items-center gap-2 font-mono">
                                      {e.to}{e.ports?.length ? ` · ${e.ports.join(', ')}` : ' · any port'}
                                      <button
                                        type="button"
                                        className="text-[var(--accent,#0071e3)] hover:underline font-sans"
                                        onClick={() => void save(r.project, { ...s, egress_allow: s.egress_allow.filter((x) => x.to !== e.to) }, `Removed ${e.to}`)}
                                      >
                                        Remove
                                      </button>
                                    </li>
                                  ))}
                                </ul>
                              )}
                              <div className="flex flex-wrap items-end gap-2">
                                <Field label="Destination" htmlFor="eg-to">
                                  <input id="eg-to" className="input text-sm w-56" placeholder="203.0.113.0/24, api.example.com, world" value={dest.to} onChange={(e) => setDest({ ...dest, to: e.target.value })} />
                                </Field>
                                <Field label="Ports" htmlFor="eg-ports">
                                  <input id="eg-ports" className="input text-sm w-32" placeholder="443, 53/udp" value={dest.ports} onChange={(e) => setDest({ ...dest, ports: e.target.value })} />
                                </Field>
                                <button
                                  type="button"
                                  className="btn-primary text-sm"
                                  disabled={busy || dest.to.trim() === ''}
                                  onClick={() => {
                                    const to = dest.to.trim()
                                    const next = [...s.egress_allow.filter((x) => x.to !== to), { to, ports: splitPorts(dest.ports) }]
                                    setDest({ to: '', ports: '' })
                                    void save(r.project, { ...s, egress_restricted: true, egress_allow: next }, `Project ${r.project}: allowed ${to}`)
                                  }}
                                >
                                  Allow
                                </button>
                              </div>
                              <div className="flex flex-wrap items-end gap-2">
                                <Field label="Egress IP host" htmlFor="eg-host">
                                  <input id="eg-host" list="eg-hosts" className="input text-sm w-40" value={eip.host} onChange={(e) => setEip({ ...eip, host: e.target.value })} />
                                </Field>
                                <Field label="Egress IP" htmlFor="eg-ip">
                                  <input id="eg-ip" className="input text-sm w-36 font-mono" placeholder="203.0.113.10" value={eip.ip} onChange={(e) => setEip({ ...eip, ip: e.target.value })} />
                                </Field>
                                <button
                                  type="button"
                                  className="btn-secondary text-sm"
                                  disabled={busy || !eip.host.trim() || !eip.ip.trim()}
                                  onClick={() => {
                                    const next = { ...s.egress_ips, [eip.host.trim()]: eip.ip.trim() }
                                    setEip({ host: '', ip: '' })
                                    void save(r.project, { ...s, egress_ips: next }, `Project ${r.project}: egress IP set`)
                                  }}
                                >
                                  Set egress IP
                                </button>
                                {ips.map(([h]) => (
                                  <button
                                    key={h}
                                    type="button"
                                    className="text-xs text-[var(--accent,#0071e3)] hover:underline"
                                    onClick={() => {
                                      const next = { ...s.egress_ips }
                                      delete next[h]
                                      void save(r.project, { ...s, egress_ips: next }, `Project ${r.project}: egress IP on ${h} removed`)
                                    }}
                                  >
                                    Remove {h}
                                  </button>
                                ))}
                                <datalist id="eg-hosts">{hostNames.map((h) => <option key={h} value={h} />)}</datalist>
                              </div>
                              <p className="text-[var(--text-muted)]">The egress IP must already be configured on that host. Private, link-local, CGNAT and multicast destinations keep the VM's own address.</p>
                            </div>
                          </td>
                        </tr>
                      )}
                    </Fragment>
                  )
                })}
              </tbody>
            </table>
          </TahoeTableWrap>
        )}
      </MacGlassPanel>

      <MacGlassPanel title="Egress IPs on hosts" subtitle="What each host's machina-bpfd installed (nftables table machina_egress).">
        {hosts.length === 0 ? (
          <Empty>No host reported.</Empty>
        ) : (
          <TahoeTableWrap>
            <table className="w-full text-xs" aria-label="Egress IPs on hosts">
              <thead>
                <tr className={headRowCls}>
                  <th scope="col" className={thCls}>Host</th>
                  <th scope="col" className={thCls}>State</th>
                  <th scope="col" className={thCls}>Rules</th>
                  <th scope="col" className={thCls}>Notes</th>
                </tr>
              </thead>
              <tbody>
                {hosts.map((h) => (
                  <tr key={h.hostname} className={rowCls}>
                    <td className="py-2 pr-2 font-medium">{h.hostname}</td>
                    <td className="py-2 pr-2"><span className={statusPillClasses(h.error ? 'error' : h.active ? 'ok' : 'neutral')}>{h.error ? 'Error' : h.active ? 'Active' : 'None'}</span></td>
                    <td className="py-2 pr-2 font-mono">{h.rules.length ? h.rules.map((x) => `${x.project}: ${x.sources.length} VM address(es) → ${x.egress_ip}`).join('; ') : '—'}</td>
                    <td className="py-2 pr-2">{[...h.skipped, ...(h.error ? [h.error] : [])].join('; ') || '—'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </TahoeTableWrap>
        )}
      </MacGlassPanel>
    </div>
  )
}

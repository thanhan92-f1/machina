// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router'
import {
  createSecurityGroup,
  createSecurityGroupRule,
  deleteSecurityGroup,
  deleteSecurityGroupRule,
  getSecurityGroup,
  listSecurityGroupRules,
  listSecurityGroups,
  type NativeSecurityGroup,
  type NativeSecurityGroupRule,
} from '../api/securityGroups'
import ConfirmDialog from '../components/ConfirmDialog'
import { useToastContext } from '../contexts/ToastContext'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import { formatUserError } from '../utils/apiError'
import { statusActionLinkClasses } from '../utils/semanticColors'
import { Loader2, RefreshCw, Shield } from 'lucide-react'

// Native security groups — this feature is libvirt/SQLite-native and does not
// depend on a wired external OpenStack cloud (there's no more <OpenStackGate>
// component to gate them behind either: the daemon's external-OpenStack-client
// integration has since been fully removed).
export default function FleetCloudSecurityGroupsPage() {
  return <FleetCloudSecurityGroupsContent />
}

function FleetCloudSecurityGroupsContent() {
  const toast = useToastContext()
  const [groups, setGroups] = useState<NativeSecurityGroup[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [detail, setDetail] = useState<NativeSecurityGroup | null>(null)
  const [rules, setRules] = useState<NativeSecurityGroupRule[]>([])
  const [loading, setLoading] = useState(true)
  const [detailLoading, setDetailLoading] = useState(false)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [deleteGroupTarget, setDeleteGroupTarget] = useState<NativeSecurityGroup | null>(null)
  const [deletingGroup, setDeletingGroup] = useState(false)
  const [newSgName, setNewSgName] = useState('')
  const [creatingSg, setCreatingSg] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    try {
      const security_groups = await listSecurityGroups()
      setGroups(security_groups)
      if (security_groups.length > 0) {
        setSelectedId((prev) => prev ?? security_groups[0].id)
      }
    } catch (e: unknown) {
      const msg = formatUserError(e)
      setLoadError(msg)
      toast.error(`Failed to load security groups: ${msg}`)
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => {
    void load()
  }, [load])

  const loadDetail = useCallback(async (id: string) => {
    const [g, r] = await Promise.all([getSecurityGroup(id), listSecurityGroupRules(id)])
    setDetail(g)
    setRules(r)
  }, [])

  useEffect(() => {
    if (!selectedId) {
      setDetail(null)
      setRules([])
      return
    }
    // Clear the previous group's rules immediately and guard against an
    // out-of-order response committing stale detail after a fast re-selection.
    let cancelled = false
    setDetail(null)
    setRules([])
    setDetailLoading(true)
    loadDetail(selectedId)
      .catch((e: unknown) => {
        if (cancelled) return
        toast.error(formatUserError(e))
        setDetail(null)
      })
      .finally(() => { if (!cancelled) setDetailLoading(false) })
    return () => { cancelled = true }
  }, [selectedId, loadDetail, toast])

  const active = detail ?? groups.find((g) => g.id === selectedId) ?? null

  return (
    <PageLayout
      hideHeader
      className="max-w-5xl"
      prepend={<><FleetCloudSubNav /></>}
      error={loadError}
      errorTitle="Failed to load"
      technicalDetail={loadError}
      errorTone="red"
      onErrorRetry={() => void load()}
      onErrorDismiss={() => setLoadError(null)}
    >
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold flex items-center gap-2">
          <Shield className="w-7 h-7 text-sky-400" />
          Security groups
        </h1>
        <button
          type="button"
          onClick={() => void load()}
          className="inline-flex items-center gap-2 px-3 py-2 rounded-lg border border-slate-600 text-slate-300 text-sm hover:bg-slate-800"
        >
          <RefreshCw className="w-4 h-4" />
          Refresh
        </button>
      </div>
      <p className="text-xs text-amber-400/90 -mt-2">
        Advisory only — rule enforcement isn't wired to the firewall yet.
      </p>

      <div className="rounded-xl border border-slate-700 p-4 flex flex-wrap gap-2 items-end text-sm">
        <input id="new-sg-name" placeholder="New group name" aria-label="New security group name"
          value={newSgName} onChange={(e) => setNewSgName(e.target.value)}
          className="px-2 py-1.5 rounded-lg bg-slate-900 border border-slate-700" />
        <button type="button" disabled={creatingSg || !newSgName.trim()}
          className="px-3 py-1.5 rounded-lg bg-sky-600 text-white disabled:opacity-40 disabled:cursor-not-allowed"
          onClick={async () => {
            const name = newSgName.trim()
            if (!name || creatingSg) return
            setCreatingSg(true)
            try {
              await createSecurityGroup({ name })
              toast.success('Security group created')
              setNewSgName('')
              void load()
            } catch (e: unknown) {
              toast.error(formatUserError(e))
            } finally {
              setCreatingSg(false)
            }
          }}>{creatingSg ? 'Creating…' : 'Create group'}</button>
        <Link to="/fleet-cloud/instances" className="text-sky-400 hover:underline ml-auto text-xs">
          Attach on instance detail
        </Link>
      </div>

      {loading ? (
        <div className="py-12 text-center text-slate-500 flex flex-col items-center gap-3">
          <Loader2 className="w-8 h-8 animate-spin text-sky-400" />
          Loading…
        </div>
      ) : groups.length === 0 ? (
        <p className="text-slate-500 text-sm">No security groups in this project.</p>
      ) : (
        <div className="grid lg:grid-cols-[minmax(12rem,16rem)_1fr] gap-6">
          <ul className="space-y-1 rounded-xl border border-slate-700 p-2 max-h-[28rem] overflow-y-auto">
            {groups.map((g) => (
              <li key={g.id}>
                <button
                  type="button"
                  onClick={() => setSelectedId(g.id)}
                  className={`w-full text-left px-3 py-2 rounded-lg text-sm ${
                    selectedId === g.id
                      ? 'bg-sky-600/20 text-sky-200 border border-sky-500/40'
                      : 'text-slate-300 hover:bg-slate-800/60'
                  }`}
                >
                  <span className="font-medium">{g.name}</span>
                  <span className="block text-xs text-slate-500 truncate">{g.id}</span>
                </button>
              </li>
            ))}
          </ul>

          <div className="rounded-xl border border-slate-700 p-4 min-h-[12rem]">
            {detailLoading && (
              <Loader2 className="w-5 h-5 animate-spin text-sky-400 mb-2" />
            )}
            {active ? (
              <>
                <div className="flex flex-wrap items-start justify-between gap-2">
                  <h2 className="text-lg font-medium text-slate-100">
                    <Link to={`/fleet-cloud/security-groups/${active.id}`} className="text-sky-300 hover:underline">{active.name}</Link>
                  </h2>
                  <button
                    type="button"
                    className={statusActionLinkClasses('error', 'text-xs')}
                    onClick={() => setDeleteGroupTarget(active)}
                  >
                    Delete group
                  </button>
                </div>
                {active.description && (
                  <p className="text-sm text-slate-500 mt-1">{active.description}</p>
                )}
                <p className="text-xs font-mono text-slate-600 mt-2 break-all">{active.id}</p>
                <h3 className="text-sm font-medium text-slate-400 mt-4 mb-2">
                  Rules ({rules.length})
                </h3>
                {selectedId && (
                  <div className="mb-4 flex flex-wrap gap-2 items-end text-xs">
                    <button type="button" className="px-2 py-1 rounded border border-slate-600"
                      onClick={async () => {
                        try {
                          await createSecurityGroupRule(selectedId, {
                            direction: 'ingress',
                            protocol: 'tcp',
                            port_min: 22,
                            port_max: 22,
                            remote_cidr: '0.0.0.0/0',
                          })
                          toast.success('SSH rule added')
                          void loadDetail(selectedId)
                        } catch (e: unknown) {
                          toast.error(formatUserError(e))
                        }
                      }}>+ SSH (22)</button>
                  </div>
                )}
                {rules.length === 0 ? (
                  <p className="text-sm text-slate-500">No rules defined.</p>
                ) : (
                  <div className="overflow-x-auto rounded-lg border border-slate-800">
                    <table className="w-full text-sm" aria-label="Security group rules">
                      <thead className="bg-slate-900 text-slate-400 text-left">
                        <tr>
                          <th scope="col" className="px-3 py-2">Direction</th>
                          <th scope="col" className="px-3 py-2">Protocol</th>
                          <th scope="col" className="px-3 py-2">Ports</th>
                          <th scope="col" className="px-3 py-2">Remote CIDR</th>
                          <th scope="col" className="px-3 py-2" />
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-slate-800 font-mono text-xs">
                        {rules.map((r) => (
                          <tr key={r.id}>
                            <td className="px-3 py-2 text-slate-300">{r.direction}</td>
                            <td className="px-3 py-2">{r.protocol || '—'}</td>
                            <td className="px-3 py-2">
                              {r.port_min != null
                                ? r.port_min === r.port_max
                                  ? String(r.port_min)
                                  : `${r.port_min}–${r.port_max}`
                                : '—'}
                            </td>
                            <td className="px-3 py-2 text-slate-400">
                              {r.remote_cidr || '—'}
                            </td>
                            <td className="px-3 py-2">
                              <button type="button" className={statusActionLinkClasses('error')}
                                onClick={async () => {
                                  try {
                                    await deleteSecurityGroupRule(r.id)
                                    toast.success('Rule deleted')
                                    void loadDetail(selectedId!)
                                  } catch (e: unknown) {
                                    toast.error(formatUserError(e))
                                  }
                                }}>Del</button>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </>
            ) : (
              <p className="text-slate-500 text-sm">Select a security group.</p>
            )}
          </div>
        </div>
      )}

      <FleetCloudFooter />

      <ConfirmDialog
        open={!!deleteGroupTarget}
        title="Delete security group"
        message={`Delete security group "${deleteGroupTarget?.name}" and all its rules?`}
        confirmLabel={deletingGroup ? 'Deleting…' : 'Delete'}
        variant="danger"
        onCancel={() => setDeleteGroupTarget(null)}
        onConfirm={async () => {
          if (!deleteGroupTarget) return
          setDeletingGroup(true)
          try {
            await deleteSecurityGroup(deleteGroupTarget.id)
            toast.success('Security group deleted')
            setDeleteGroupTarget(null)
            setSelectedId(null)
            setDetail(null)
            void load()
          } catch (e: unknown) {
            toast.error(formatUserError(e))
          } finally {
            setDeletingGroup(false)
          }
        }}
      />
    </PageLayout>
  )
}

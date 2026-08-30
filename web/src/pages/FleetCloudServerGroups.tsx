// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router'
import { listVms, type NativeVm } from '../api/nativeVms'
import { addVmToServerGroup, deleteServerGroup, listServerGroups, type DerivedServerGroup } from '../api/nativeServerGroups'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { statusActionLinkClasses } from '../utils/semanticColors'
import { Layers, Loader2, RefreshCw } from 'lucide-react'

// Native anti-affinity groups — no old external-cloud gate component in the
// way any more (the daemon's external-cloud-client integration has
// since been fully removed). A "group" is derived from VM tags (see
// api/nativeServerGroups.ts), not a stored resource — only anti-affinity is
// supported (Machina's placement engine only enforces that policy).
export default function FleetCloudServerGroupsPage() {
  return <FleetCloudServerGroupsContent />
}

function FleetCloudServerGroupsContent() {
  const toast = useToastContext()
  const [groups, setGroups] = useState<DerivedServerGroup[]>([])
  const [vms, setVms] = useState<NativeVm[]>([])
  const [loading, setLoading] = useState(true)
  const [groupName, setGroupName] = useState('')
  const [vmId, setVmId] = useState('')

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const [g, v] = await Promise.all([listServerGroups(), listVms()])
      setGroups(g)
      setVms(v)
      if (!vmId && v.length > 0) setVmId(v[0].id)
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    } finally {
      setLoading(false)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [toast])

  useEffect(() => {
    void load()
  }, [load])

  return (
    <PageLayout
      hideHeader
      className="w-full max-w-none"
      prepend={<><FleetCloudSubNav /></>}
    >
      <p className="apple-eyebrow">Fleet Cloud</p>
      <h1 className="page-title flex items-center gap-3">
        <Layers className="w-7 h-7 text-[var(--accent)]" />
        Anti-affinity groups
      </h1>
      <p className="text-sm text-[var(--text-muted)]">
        Machina's placement engine avoids co-locating VMs that share a group on the same host.
      </p>
      <div className="rounded-2xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)] p-4 flex flex-wrap gap-3 items-end">
        <div>
          <label className="block text-xs text-[var(--text-muted)] mb-1">Group name</label>
          <input value={groupName} onChange={(e) => setGroupName(e.target.value)}
            aria-label="Group name"
            className="input-field text-sm" />
        </div>
        <div>
          <label className="block text-xs text-[var(--text-muted)] mb-1">VM</label>
          <select value={vmId} onChange={(e) => setVmId(e.target.value)}
            aria-label="VM"
            className="input-field text-sm">
            {vms.map((v) => <option key={v.id} value={v.id}>{v.name}</option>)}
          </select>
        </div>
        <button type="button" disabled={!groupName.trim() || !vmId}
          className="btn-primary text-sm disabled:opacity-40"
          onClick={async () => {
            const vm = vms.find((v) => v.id === vmId)
            if (!vm) return
            try {
              await addVmToServerGroup(vm, groupName.trim())
              toast.success(`Added '${vm.name}' to group '${groupName.trim()}'`)
              setGroupName('')
              void load()
            } catch (e: unknown) {
              toast.error(formatUserError(e))
            }
          }}>
          Add to group
        </button>
      </div>
      <button type="button" onClick={() => void load()}
        className="btn-secondary text-sm inline-flex items-center gap-1">
        <RefreshCw className="w-4 h-4" /> Refresh
      </button>
      {loading ? (
        <Loader2 className="w-8 h-8 animate-spin text-[var(--accent)]" />
      ) : (
        <ul className="rounded-2xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)] divide-y divide-[var(--apple-hairline)]">
          {groups.map((g) => (
            <li key={g.name} className="px-4 py-3 flex flex-wrap justify-between gap-2 text-sm">
              <div>
                <Link to={`/fleet-cloud/server-groups/${encodeURIComponent(g.name)}`} className="font-mono text-[var(--text-primary)] hover:opacity-90 hover:underline">{g.name}</Link>
                <span className="ml-2 text-xs text-[var(--text-muted)]">anti-affinity</span>
                {g.members.length > 0 && (
                  <span className="block text-xs text-[var(--text-muted)] mt-1">{g.members.length} member(s)</span>
                )}
              </div>
              <button type="button" className={statusActionLinkClasses('error', 'text-xs self-start')}
                onClick={async () => {
                  if (!confirm(`Delete group ${g.name}? This removes the tag from all ${g.members.length} member VM(s).`)) return
                  try {
                    await deleteServerGroup(g)
                    toast.success('Deleted')
                    void load()
                  } catch (e: unknown) {
                    toast.error(formatUserError(e))
                  }
                }}>Delete</button>
            </li>
          ))}
          {groups.length === 0 && (
            <li className="px-4 py-6 text-center text-[var(--text-muted)]">No anti-affinity groups yet.</li>
          )}
        </ul>
      )}
      <FleetCloudFooter />
    </PageLayout>
  )
}

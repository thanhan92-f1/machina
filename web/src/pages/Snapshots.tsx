// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useEffect, useState, useCallback } from 'react'
import { listAllSnapshots, deleteSnapshot, revertSnapshot, SnapshotInfo } from '../api/snapshot'
import { useToastContext } from '../contexts/ToastContext'
import ConfirmDialog from '../components/ConfirmDialog'
import EmptyState from '../components/EmptyState'
import PageLayout from '../components/PageLayout'
import { Trash2, RotateCcw, RefreshCw, Camera, AlertTriangle } from 'lucide-react'
import { formatUserError } from '../utils/apiError'
import { statusActionLinkClasses, statusBadgeClasses, statusToneClass } from '../utils/semanticColors'
import { snapshotStateSeverity } from '../utils/snapshotHealth'

function SnapshotStateBadge({ state }: { state: string }) {
  const sev = snapshotStateSeverity(state)
  return (
    <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium ${statusBadgeClasses(sev)}`}>
      {(sev === 'error' || sev === 'warn') && <AlertTriangle className="w-3 h-3" />}
      {state || '—'}
    </span>
  )
}

export default function SnapshotsPage() {
  const [snapshots, setSnapshots] = useState<SnapshotInfo[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<SnapshotInfo | null>(null)
  const [revertTarget, setRevertTarget] = useState<SnapshotInfo | null>(null)
  const toast = useToastContext()

  const load = useCallback(async () => {
    try {
      setLoading(true)
      setLoadError(null)
      setSnapshots(await listAllSnapshots())
    } catch (e: unknown) {
      setLoadError(formatUserError(e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { load() }, [load])

  const handleRevert = async () => {
    if (!revertTarget) return
    const snap = revertTarget
    setRevertTarget(null)
    try { await revertSnapshot(snap.vm_name, snap.name); toast.success(`Reverted '${snap.vm_name}' to '${snap.name}'`); load() } catch (e: unknown) { toast.error(`${formatUserError(e)}`) }
  }

  const handleDelete = async () => {
    if (!deleteTarget) return
    try { await deleteSnapshot(deleteTarget.vm_name, deleteTarget.name); toast.success(`Deleted snapshot '${deleteTarget.name}'`); load() } catch (e: unknown) { toast.error(`${formatUserError(e)}`) }
    setDeleteTarget(null)
  }

  return (
    <PageLayout
      eyebrow="Hypervisor"
      title="Snapshots"
      icon={<Camera className="w-6 h-6" />}
      actions={
        <button onClick={load} className="p-2 hover:bg-[var(--surface-hover)] rounded transition" title="Refresh" aria-label="Refresh">
          <RefreshCw className="w-4 h-4" />
        </button>
      }
      contentLoading={loading}
      error={loadError}
      errorTitle="Failed to load snapshots"
      onErrorRetry={load}
      onErrorDismiss={() => setLoadError(null)}
    >
      {loadError ? null : snapshots.length === 0 ? (
        <EmptyState
          icon={<Camera className="w-6 h-6" />}
          title="No snapshots"
          description="VM snapshots appear here after you create them from a guest's details page."
        />
      ) : (
        <div className="bg-[var(--apple-surface)] rounded-2xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)]/50 overflow-hidden">
          <table className="w-full" aria-label="VM snapshots">
            <thead><tr className="border-b border-[var(--apple-hairline)] text-left text-sm text-[var(--text-muted)]"><th scope="col" className="px-6 py-3">Snapshot</th><th scope="col" className="px-6 py-3">VM</th><th scope="col" className="px-6 py-3">State</th><th scope="col" className="px-6 py-3 hidden md:table-cell">Created</th><th scope="col" className="px-6 py-3">Current</th><th scope="col" className="px-6 py-3 text-right">Actions</th></tr></thead>
            <tbody className="divide-y divide-[var(--apple-hairline)]/50">
              {snapshots.map((s) => (
                <tr key={`${s.vm_name}/${s.name}`} className="hover:bg-[var(--surface-hover)]/50">
                  <td className="px-6 py-3 font-medium">{s.name}</td>
                  <td className={`px-6 py-3 text-sm ${statusActionLinkClasses('info')}`}>{s.vm_name}</td>
                  <td className="px-6 py-3"><SnapshotStateBadge state={s.state} /></td>
                  <td className="px-6 py-3 text-sm text-[var(--text-muted)] hidden md:table-cell">{s.creation_time ? new Date(s.creation_time * 1000).toLocaleString() : '-'}</td>
                  <td className="px-6 py-3">{s.is_current && <span className={`text-xs font-medium ${statusToneClass('ok')}`}>● Current</span>}</td>
                  <td className="px-6 py-3">
                    <div className="flex items-center justify-end gap-1">
                      <button onClick={() => setRevertTarget(s)} className="p-1.5 hover:bg-white/10 rounded transition" title="Revert" aria-label="Revert"><RotateCcw className={`w-4 h-4 ${statusToneClass('info')}`} /></button>
                      <button onClick={() => setDeleteTarget(s)} className="p-1.5 hover:bg-red-600/20 rounded transition" title="Delete" aria-label="Delete"><Trash2 className={`w-4 h-4 ${statusToneClass('error')}`} /></button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <ConfirmDialog open={!!deleteTarget} title="Delete Snapshot" message={`Delete snapshot '${deleteTarget?.name}' from VM '${deleteTarget?.vm_name}'?`} confirmLabel="Delete" onConfirm={handleDelete} onCancel={() => setDeleteTarget(null)} />
      <ConfirmDialog open={!!revertTarget} variant="warning" title="Revert Snapshot" message={`Revert VM '${revertTarget?.vm_name}' to snapshot '${revertTarget?.name}'? The guest's current disk and memory state will be discarded.`} confirmLabel="Revert" onConfirm={handleRevert} onCancel={() => setRevertTarget(null)} />
    </PageLayout>
  )
}

// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useCallback, useEffect, useRef, useState } from 'react'
import { Link } from 'react-router'
import { Box, Layers, Play, Plus, RefreshCw, Square, Trash2, X } from 'lucide-react'
import {
  createVesselPod,
  getVesselStatus,
  listVesselPods,
  removeVesselPod,
  shortId,
  startVesselPod,
  stopVesselPod,
  type PodSummary,
  type VesselStatus,
} from '../api/vessel'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { useFocusTrap } from '../hooks/useFocusTrap'
import PageLayout from '../components/PageLayout'
import EmptyState from '../components/EmptyState'
import ConfirmDialog from '../components/ConfirmDialog'
import { statusBadgeClasses } from '../utils/semanticColors'

function podTone(status: string): 'ok' | 'warn' | 'error' | 'neutral' | 'info' {
  const s = status.toLowerCase()
  if (s === 'running') return 'ok'
  if (s === 'degraded' || s === 'paused') return 'warn'
  if (s === 'dead' || s === 'exited' || s === 'stopped') return 'error'
  if (s === 'created') return 'info'
  return 'neutral'
}

export default function ContainerPodsPage() {
  const toast = useToastContext()
  const [status, setStatus] = useState<VesselStatus | null>(null)
  const [items, setItems] = useState<PodSummary[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<PodSummary | null>(null)
  const [showCreate, setShowCreate] = useState(false)
  const [newName, setNewName] = useState('')
  const [creating, setCreating] = useState(false)
  const createRef = useRef<HTMLDivElement>(null)
  useFocusTrap(createRef, showCreate, () => setShowCreate(false))

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const st = await getVesselStatus()
      setStatus(st)
      if (!st.connected) {
        setItems([])
        setError(st.error || 'Container engine not connected')
        return
      }
      if (!st.capabilities?.pods) {
        setItems([])
        setError('Podman pods require a Podman engine (Docker does not support pods).')
        return
      }
      const list = await listVesselPods()
      setItems(list)
    } catch (e) {
      setError(formatUserError(e))
      setItems([])
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const runAction = async (id: string, label: string, fn: () => Promise<unknown>) => {
    setBusy(id)
    try {
      await fn()
      toast.success(`${label} ok`)
      await load()
    } catch (e) {
      toast.error(formatUserError(e))
    } finally {
      setBusy(null)
    }
  }

  const onCreate = async () => {
    const name = newName.trim()
    if (!name) {
      toast.error('Pod name is required')
      return
    }
    setCreating(true)
    try {
      await createVesselPod(name)
      toast.success(`Pod ${name} created`)
      setShowCreate(false)
      setNewName('')
      await load()
    } catch (e) {
      toast.error(formatUserError(e))
    } finally {
      setCreating(false)
    }
  }

  return (
    <PageLayout
      eyebrow="Vessel"
      title="Container Pods"
      subtitle="Podman pods on this host — shared network namespace for grouped containers."
      icon={<Layers className="w-5 h-5" />}
      loading={loading && items.length === 0 && !error}
      error={error}
      errorTitle="Pods unavailable"
      errorHints={[
        'Pods require Podman (not Docker)',
        'Use Containers for Docker/Podman container lifecycle',
        'Kubernetes pods are under /k8s/workloads',
      ]}
      onErrorRetry={() => void load()}
      actions={
        <div className="flex items-center gap-2">
          <Link
            to="/containers"
            className="inline-flex items-center gap-1.5 btn-secondary text-sm text-[var(--text-primary)] hover:bg-[var(--apple-fill-tertiary)]"
          >
            <Box className="w-3.5 h-3.5" /> Containers
          </Link>
          <button
            type="button"
            disabled={!status?.capabilities?.pods}
            onClick={() => setShowCreate(true)}
            className="btn-primary text-sm disabled:opacity-40"
          >
            <Plus className="w-3.5 h-3.5" /> Create pod
          </button>
          <button
            type="button"
            onClick={() => void load()}
            className="inline-flex items-center gap-1.5 btn-secondary text-sm text-[var(--text-primary)] hover:bg-[var(--apple-fill-tertiary)]"
          >
            <RefreshCw className="w-3.5 h-3.5" /> Refresh
          </button>
        </div>
      }
    >
      {!loading && !error && items.length === 0 && (
        <EmptyState
          icon={<Layers className="w-8 h-8" />}
          title="No pods"
          description="Create a Podman pod to group containers with a shared network namespace."
          primaryAction={
            <button
              type="button"
              onClick={() => setShowCreate(true)}
              className="btn-primary text-sm"
            >
              Create pod
            </button>
          }
        />
      )}

      {items.length > 0 && (
        <div className="overflow-x-auto rounded-xl border border-white/10">
          <table className="w-full text-sm">
            <thead className="bg-[var(--apple-surface)] text-left text-[var(--text-muted)]">
              <tr>
                <th className="px-3 py-2 font-medium">Name</th>
                <th className="px-3 py-2 font-medium">Status</th>
                <th className="px-3 py-2 font-medium">Containers</th>
                <th className="px-3 py-2 font-medium">ID</th>
                <th className="px-3 py-2 font-medium text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {items.map((p) => {
                const tone = podTone(p.status)
                const disabled = busy === p.id
                return (
                  <tr key={p.id} className="border-t border-white/5 hover:bg-white/[0.03]">
                    <td className="px-3 py-2 text-[var(--text-primary)] font-medium">{p.name}</td>
                    <td className="px-3 py-2">
                      <span className={statusBadgeClasses(tone)}>{p.status}</span>
                    </td>
                    <td className="px-3 py-2 text-[var(--text-muted)]">{p.containers.length}</td>
                    <td className="px-3 py-2 font-mono text-xs text-[var(--text-muted)]">{shortId(p.id)}</td>
                    <td className="px-3 py-2">
                      <div className="flex justify-end gap-1">
                        <button
                          type="button"
                          disabled={disabled}
                          title="Start"
                          onClick={() => void runAction(p.id, 'Start', () => startVesselPod(p.id))}
                          className="p-1.5 rounded-lg hover:bg-emerald-500/20 text-emerald-400 disabled:opacity-40"
                        >
                          <Play className="w-3.5 h-3.5" />
                        </button>
                        <button
                          type="button"
                          disabled={disabled}
                          title="Stop"
                          onClick={() => void runAction(p.id, 'Stop', () => stopVesselPod(p.id))}
                          className="p-1.5 rounded-lg hover:bg-amber-500/20 text-amber-400 disabled:opacity-40"
                        >
                          <Square className="w-3.5 h-3.5" />
                        </button>
                        <button
                          type="button"
                          disabled={disabled}
                          title="Remove"
                          onClick={() => setDeleteTarget(p)}
                          className="p-1.5 rounded-lg hover:bg-rose-500/20 text-rose-400 disabled:opacity-40"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}

      {showCreate && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
          <div
            ref={createRef}
            className="w-full max-w-md rounded-2xl border border-white/10 bg-[var(--apple-surface)] p-5 shadow-xl"
            role="dialog"
            aria-modal="true"
            aria-labelledby="create-pod-title"
          >
            <div className="flex items-center justify-between mb-4">
              <h2 id="create-pod-title" className="text-lg font-semibold text-[var(--text-primary)]">
                Create Podman pod
              </h2>
              <button
                type="button"
                onClick={() => setShowCreate(false)}
                className="p-1 rounded-lg hover:bg-white/10 text-[var(--text-muted)]"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <label className="block text-sm text-[var(--text-muted)] mb-1" htmlFor="pod-name">
              Name
            </label>
            <input
              id="pod-name"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              className="w-full rounded-lg border border-[var(--apple-hairline)] bg-[var(--apple-surface)] px-3 py-2 text-[var(--text-primary)] mb-4"
              placeholder="my-pod"
              autoFocus
            />
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setShowCreate(false)}
                className="btn-secondary text-sm text-[var(--text-secondary)]"
              >
                Cancel
              </button>
              <button
                type="button"
                disabled={creating}
                onClick={() => void onCreate()}
                className="btn-primary text-sm disabled:opacity-40"
              >
                {creating ? 'Creating…' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}

      <ConfirmDialog
        open={deleteTarget !== null}
        title="Remove pod"
        message={deleteTarget ? `Force-remove pod “${deleteTarget.name}”?` : ''}
        confirmLabel="Remove"
        variant="danger"
        onCancel={() => setDeleteTarget(null)}
        onConfirm={() => {
          const t = deleteTarget
          setDeleteTarget(null)
          if (!t) return
          void runAction(t.id, 'Remove', () => removeVesselPod(t.id, true))
        }}
      />
    </PageLayout>
  )
}

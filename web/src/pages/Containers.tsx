// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useSearchParams } from 'react-router'
import {
  Box,
  Play,
  Plus,
  RefreshCw,
  Square,
  Trash2,
  RotateCw,
  Layers,
  X,
} from 'lucide-react'
import {
  createVesselContainer,
  getVesselStatus,
  listVesselContainers,
  reconnectVessel,
  removeVesselContainer,
  restartVesselContainer,
  shortId,
  startVesselContainer,
  stopVesselContainer,
  type ContainerSummary,
  type VesselStatus,
} from '../api/vessel'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { useFocusTrap } from '../hooks/useFocusTrap'
import PageLayout from '../components/PageLayout'
import EmptyState from '../components/EmptyState'
import ConfirmDialog from '../components/ConfirmDialog'
import { statusBadgeClasses, statusToneClass } from '../utils/semanticColors'

function statusTone(status: string): 'ok' | 'warn' | 'error' | 'neutral' | 'info' {
  const s = status.toLowerCase()
  if (s === 'running') return 'ok'
  if (s === 'paused' || s === 'restarting') return 'warn'
  if (s === 'dead' || s === 'exited') return 'error'
  if (s === 'created') return 'info'
  return 'neutral'
}

function parseCommand(raw: string): string[] {
  const t = raw.trim()
  if (!t) return []
  return t.split(/\s+/).filter(Boolean)
}

export default function ContainersPage() {
  const toast = useToastContext()
  const [searchParams, setSearchParams] = useSearchParams()
  const [status, setStatus] = useState<VesselStatus | null>(null)
  const [items, setItems] = useState<ContainerSummary[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState<string | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<ContainerSummary | null>(null)
  const [showCreate, setShowCreate] = useState(false)
  const [creating, setCreating] = useState(false)
  const [newName, setNewName] = useState('')
  const [newImage, setNewImage] = useState('docker.io/library/nginx:alpine')
  const [newCommand, setNewCommand] = useState('')
  const [startAfterCreate, setStartAfterCreate] = useState(true)
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
      const list = await listVesselContainers(true)
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

  useEffect(() => {
    if (searchParams.get('create') === '1' && status?.connected) {
      setShowCreate(true)
      const next = new URLSearchParams(searchParams)
      next.delete('create')
      setSearchParams(next, { replace: true })
    }
  }, [searchParams, setSearchParams, status?.connected])

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
    const image = newImage.trim()
    if (!name || !image) {
      toast.error('Name and image are required')
      return
    }
    setCreating(true)
    try {
      await createVesselContainer({
        name,
        image,
        command: parseCommand(newCommand),
        start: startAfterCreate,
      })
      toast.success(startAfterCreate ? `Container ${name} created and started` : `Container ${name} created`)
      setShowCreate(false)
      setNewName('')
      setNewCommand('')
      await load()
    } catch (e) {
      toast.error(formatUserError(e))
    } finally {
      setCreating(false)
    }
  }

  const podsCapable = Boolean(status?.capabilities?.pods)
  const engineLabel = status?.engine ? String(status.engine) : '—'
  const runningCount = items.filter((c) => String(c.status).toLowerCase() === 'running').length

  return (
    <PageLayout
      title="Containers"
      subtitle="Local Podman / Docker containers on this host (Vessel)."
      icon={<Box className="w-5 h-5" />}
      loading={loading && items.length === 0 && !error}
      error={error}
      errorTitle="Container engine unavailable"
      errorHints={[
        'Start Podman or Docker on this host',
        'Or set [vessel].socket in /etc/machina/config.toml',
        'Kubernetes pods remain under K8s Workloads',
      ]}
      onErrorRetry={() => void load()}
      actions={
        <div className="flex items-center gap-2">
          {podsCapable && (
            <Link
              to="/containers/pods"
              className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-slate-600 text-sm text-slate-200 hover:bg-slate-800"
            >
              <Layers className="w-3.5 h-3.5" /> Pods
            </Link>
          )}
          <button
            type="button"
            disabled={!status?.connected}
            onClick={() => setShowCreate(true)}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white text-sm disabled:opacity-40"
          >
            <Plus className="w-3.5 h-3.5" /> Create container
          </button>
          <button
            type="button"
            onClick={() =>
              void reconnectVessel()
                .then(() => load())
                .catch((e) => toast.error(formatUserError(e)))
            }
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-slate-600 text-sm text-slate-200 hover:bg-slate-800"
          >
            <RefreshCw className="w-3.5 h-3.5" /> Reconnect
          </button>
          <button
            type="button"
            onClick={() => void load()}
            className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-500 text-white text-sm"
          >
            <RefreshCw className="w-3.5 h-3.5" /> Refresh
          </button>
        </div>
      }
    >
      {status?.connected && (
        <div className="mb-4 rounded-xl border border-white/10 bg-white/5 px-4 py-3 text-sm text-slate-300 flex flex-wrap gap-x-6 gap-y-1">
          <span>
            Engine <strong className="text-slate-100">{engineLabel}</strong>
          </span>
          <span>
            Version <strong className="text-slate-100">{status.version || '—'}</strong>
          </span>
          <span>
            Containers <strong className="text-slate-100">{items.length}</strong>
            {items.length > 0 ? ` (${runningCount} running)` : ''}
          </span>
          <span className="font-mono text-xs text-slate-400 truncate max-w-md">
            {status.socket || '—'}
          </span>
        </div>
      )}

      {!loading && status?.connected && items.length === 0 && (
        <EmptyState
          icon={<Box className="w-8 h-8" />}
          title="No containers"
          description="Pull an image and run your first container from this page."
          primaryAction={
            <button
              type="button"
              onClick={() => setShowCreate(true)}
              className="px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white text-sm"
            >
              Create container
            </button>
          }
        />
      )}

      {items.length > 0 && (
        <div className="overflow-x-auto rounded-xl border border-white/10">
          <table className="w-full text-sm">
            <thead className="bg-slate-900/60 text-left text-slate-400">
              <tr>
                <th className="px-3 py-2 font-medium">Name</th>
                <th className="px-3 py-2 font-medium">Image</th>
                <th className="px-3 py-2 font-medium">Status</th>
                <th className="px-3 py-2 font-medium">ID</th>
                <th className="px-3 py-2 font-medium text-right">Actions</th>
              </tr>
            </thead>
            <tbody>
              {items.map((c) => {
                const tone = statusTone(c.status)
                const id = c.id
                const disabled = busy === id
                return (
                  <tr key={id} className="border-t border-white/5 hover:bg-white/[0.03]">
                    <td className="px-3 py-2 text-slate-100 font-medium">{c.name || '—'}</td>
                    <td className="px-3 py-2 text-slate-300 font-mono text-xs max-w-[14rem] truncate">
                      {c.image}
                    </td>
                    <td className="px-3 py-2">
                      <span className={statusBadgeClasses(tone)}>{c.status}</span>
                      <span className={`ml-2 text-xs ${statusToneClass(tone)}`}>{c.state}</span>
                    </td>
                    <td className="px-3 py-2 font-mono text-xs text-slate-500">{shortId(id)}</td>
                    <td className="px-3 py-2">
                      <div className="flex justify-end gap-1">
                        <button
                          type="button"
                          disabled={disabled}
                          title="Start"
                          onClick={() => void runAction(id, 'Start', () => startVesselContainer(id))}
                          className="p-1.5 rounded-lg hover:bg-emerald-500/20 text-emerald-400 disabled:opacity-40"
                        >
                          <Play className="w-3.5 h-3.5" />
                        </button>
                        <button
                          type="button"
                          disabled={disabled}
                          title="Stop"
                          onClick={() => void runAction(id, 'Stop', () => stopVesselContainer(id))}
                          className="p-1.5 rounded-lg hover:bg-amber-500/20 text-amber-400 disabled:opacity-40"
                        >
                          <Square className="w-3.5 h-3.5" />
                        </button>
                        <button
                          type="button"
                          disabled={disabled}
                          title="Restart"
                          onClick={() =>
                            void runAction(id, 'Restart', () => restartVesselContainer(id))
                          }
                          className="p-1.5 rounded-lg hover:bg-sky-500/20 text-sky-400 disabled:opacity-40"
                        >
                          <RotateCw className="w-3.5 h-3.5" />
                        </button>
                        <button
                          type="button"
                          disabled={disabled}
                          title="Remove"
                          onClick={() => setDeleteTarget(c)}
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
            className="w-full max-w-lg rounded-2xl border border-white/10 bg-slate-900 p-5 shadow-xl"
            role="dialog"
            aria-modal="true"
            aria-labelledby="create-container-title"
          >
            <div className="flex items-center justify-between mb-4">
              <h2 id="create-container-title" className="text-lg font-semibold text-slate-100">
                Create container
              </h2>
              <button
                type="button"
                onClick={() => setShowCreate(false)}
                className="p-1 rounded-lg hover:bg-white/10 text-slate-400"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="space-y-3">
              <div>
                <label className="block text-sm text-slate-400 mb-1" htmlFor="ctr-name">
                  Name
                </label>
                <input
                  id="ctr-name"
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  className="w-full rounded-lg border border-slate-600 bg-slate-950 px-3 py-2 text-slate-100"
                  placeholder="my-app"
                  autoFocus
                />
              </div>
              <div>
                <label className="block text-sm text-slate-400 mb-1" htmlFor="ctr-image">
                  Image
                </label>
                <input
                  id="ctr-image"
                  value={newImage}
                  onChange={(e) => setNewImage(e.target.value)}
                  className="w-full rounded-lg border border-slate-600 bg-slate-950 px-3 py-2 text-slate-100 font-mono text-sm"
                  placeholder="docker.io/library/nginx:alpine"
                />
              </div>
              <div>
                <label className="block text-sm text-slate-400 mb-1" htmlFor="ctr-cmd">
                  Command <span className="text-slate-500">(optional)</span>
                </label>
                <input
                  id="ctr-cmd"
                  value={newCommand}
                  onChange={(e) => setNewCommand(e.target.value)}
                  className="w-full rounded-lg border border-slate-600 bg-slate-950 px-3 py-2 text-slate-100 font-mono text-sm"
                  placeholder="nginx -g daemon off;"
                />
              </div>
              <label className="flex items-center gap-2 text-sm text-slate-300">
                <input
                  type="checkbox"
                  checked={startAfterCreate}
                  onChange={(e) => setStartAfterCreate(e.target.checked)}
                  className="rounded border-slate-600"
                />
                Start after create
              </label>
            </div>
            <div className="flex justify-end gap-2 mt-5">
              <button
                type="button"
                onClick={() => setShowCreate(false)}
                className="px-3 py-1.5 rounded-lg border border-slate-600 text-sm text-slate-300"
              >
                Cancel
              </button>
              <button
                type="button"
                disabled={creating}
                onClick={() => void onCreate()}
                className="px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-white text-sm disabled:opacity-40"
              >
                {creating ? 'Creating…' : 'Create'}
              </button>
            </div>
          </div>
        </div>
      )}

      <ConfirmDialog
        open={deleteTarget !== null}
        title="Remove container"
        message={
          deleteTarget
            ? `Force-remove container “${deleteTarget.name || shortId(deleteTarget.id)}”?`
            : ''
        }
        confirmLabel="Remove"
        variant="danger"
        onCancel={() => setDeleteTarget(null)}
        onConfirm={() => {
          const t = deleteTarget
          setDeleteTarget(null)
          if (!t) return
          void runAction(t.id, 'Remove', () => removeVesselContainer(t.id, true))
        }}
      />
    </PageLayout>
  )
}

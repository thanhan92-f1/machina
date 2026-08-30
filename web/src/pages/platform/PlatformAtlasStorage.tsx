// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Atlas — Zyvor storage control plane console. Browse backends/volumes/
// snapshots/backups provisioned through Atlas and drive the write path.

import { useCallback, useEffect, useState } from 'react'
import {
  Boxes,
  Camera,
  Database,
  HardDrive,
  Layers,
  Loader2,
  Plus,
  RefreshCw,
  Server,
  Trash2,
} from 'lucide-react'
import PlatformPageChrome from '../../components/platform/PlatformPageChrome'
import { MacGlassPanel, MacStatWidget } from '../../components/platform/mac/PlatformMacUi'
import { useToastContext } from '../../contexts/ToastContext'
import { formatUserError } from '../../utils/apiError'
import {
  backupAtlasVolume,
  createAtlasVolume,
  deleteAtlasBackup,
  deleteAtlasSnapshot,
  deleteAtlasVolume,
  getAtlasStatus,
  listAtlasBackends,
  listAtlasBackups,
  listAtlasJobs,
  listAtlasSnapshots,
  listAtlasVolumes,
  restoreAtlasSnapshot,
  snapshotAtlasVolume,
  type AtlasBackend,
  type AtlasBackup,
  type AtlasJob,
  type AtlasSnapshot,
  type AtlasStatus,
  type AtlasVolume,
} from '../../api/platformAtlas'

const TABS = ['Backends', 'Volumes', 'Snapshots', 'Backups', 'Jobs'] as const
type Tab = (typeof TABS)[number]

function fmtBytes(n?: number | null): string {
  if (n == null) return '—'
  const gib = n / (1024 * 1024 * 1024)
  if (gib >= 1) return `${gib.toFixed(gib >= 10 ? 0 : 1)} GiB`
  const mib = n / (1024 * 1024)
  return `${mib.toFixed(0)} MiB`
}

function StatePill({ value }: { value?: string | null }) {
  const v = (value || 'unknown').toLowerCase()
  const tone =
    v === 'available' || v === 'ready' || v === 'bound' || v === 'verified' || v === 'succeeded'
      ? 'text-emerald-400 bg-emerald-500/10'
      : v === 'failed'
        ? 'text-rose-400 bg-rose-500/10'
        : 'text-amber-400 bg-amber-500/10'
  return <span className={`text-xs px-2 py-0.5 rounded-full ${tone}`}>{value || 'unknown'}</span>
}

export default function PlatformAtlasStorage() {
  const toast = useToastContext()
  const [tab, setTab] = useState<Tab>('Volumes')
  const [status, setStatus] = useState<AtlasStatus | null>(null)
  const [backends, setBackends] = useState<AtlasBackend[]>([])
  const [volumes, setVolumes] = useState<AtlasVolume[]>([])
  const [snapshots, setSnapshots] = useState<AtlasSnapshot[]>([])
  const [backups, setBackups] = useState<AtlasBackup[]>([])
  const [jobs, setJobs] = useState<AtlasJob[]>([])
  const [loading, setLoading] = useState(true)
  const [busy, setBusy] = useState<string | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const st = await getAtlasStatus()
      setStatus(st)
      if (!st.enabled || !st.reachable) return
      const [b, v, s, bk, j] = await Promise.all([
        listAtlasBackends().catch(() => []),
        listAtlasVolumes().catch(() => []),
        listAtlasSnapshots().catch(() => []),
        listAtlasBackups().catch(() => []),
        listAtlasJobs().catch(() => []),
      ])
      setBackends(b)
      setVolumes(v)
      setSnapshots(s)
      setBackups(bk)
      setJobs(j)
    } catch (e) {
      toast.error(formatUserError(e))
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => {
    void load()
  }, [load])

  const run = async (key: string, fn: () => Promise<unknown>, ok: string) => {
    setBusy(key)
    try {
      await fn()
      toast.success(ok)
      await load()
    } catch (e) {
      toast.error(formatUserError(e))
    } finally {
      setBusy(null)
    }
  }

  const onCreateVolume = () => {
    const name = window.prompt('New volume name')
    if (!name) return
    const sizeGib = Number(window.prompt('Size (GiB)', '10'))
    if (!sizeGib || sizeGib <= 0) return
    void run(
      'create',
      () => createAtlasVolume({ name, size_bytes: Math.round(sizeGib * 1024 * 1024 * 1024) }),
      `Volume ${name} creating`,
    )
  }

  const disabled = status && !status.enabled
  const unreachable = status && status.enabled && !status.reachable

  return (
    <PlatformPageChrome
      eyebrow="Platform"
      title="Storage (Atlas)"
      subtitle={status?.summary ?? 'Zyvor storage control plane'}
      icon={<Database className="w-6 h-6 text-[var(--text-muted)]" />}
      actions={
        <div className="flex items-center gap-2">
          <button type="button" className="btn-secondary" onClick={() => void load()} aria-label="Refresh">
            <RefreshCw className="w-4 h-4" />
          </button>
          <button
            type="button"
            className="btn-primary inline-flex items-center gap-1.5"
            onClick={onCreateVolume}
            disabled={!!disabled || !!unreachable || busy === 'create'}
          >
            <Plus className="w-4 h-4" /> New volume
          </button>
        </div>
      }
    >
      {loading ? (
        <div className="flex items-center justify-center py-20 text-[var(--text-muted)]">
          <Loader2 className="w-5 h-5 animate-spin mr-2" /> Loading Atlas…
        </div>
      ) : disabled ? (
        <MacGlassPanel title="Atlas disabled">
          <p className="text-sm text-[var(--text-muted)]">
            The Atlas storage integration is disabled. Set <code>ATLAS_ENABLED=1</code> and{' '}
            <code>ATLAS_BASE_URL</code> on the controller to connect a Zyvor storage control plane.
          </p>
        </MacGlassPanel>
      ) : unreachable ? (
        <MacGlassPanel title="Atlas unreachable">
          <p className="text-sm text-[var(--text-muted)]">
            Could not reach the Atlas gateway at <code>{status?.base_url}</code>. Check the gateway and
            <code> ATLAS_BASE_URL</code>.
          </p>
        </MacGlassPanel>
      ) : (
        <div className="space-y-4">
          <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
            <MacStatWidget label="Backends" value={String(backends.length)} icon={<Server className="w-4 h-4" />} />
            <MacStatWidget label="Volumes" value={String(volumes.length)} icon={<HardDrive className="w-4 h-4" />} />
            <MacStatWidget label="Snapshots" value={String(snapshots.length)} icon={<Camera className="w-4 h-4" />} />
            <MacStatWidget label="Backups" value={String(backups.length)} icon={<Boxes className="w-4 h-4" />} />
            <MacStatWidget
              label="Auth"
              value={status?.authenticated ? 'JWT' : 'open'}
              icon={<Layers className="w-4 h-4" />}
              tone={status?.authenticated ? 'ok' : 'default'}
            />
          </div>

          <div className="flex items-center gap-1 border-b border-white/[0.06]">
            {TABS.map((t) => (
              <button
                key={t}
                type="button"
                onClick={() => setTab(t)}
                className={`px-3 py-2 text-sm transition ${
                  tab === t
                    ? 'text-[var(--text-primary)] border-b-2 border-[var(--accent)]'
                    : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)]'
                }`}
              >
                {t}
              </button>
            ))}
          </div>

          {tab === 'Backends' && (
            <MacGlassPanel title="Storage backends" subtitle="Registered Atlas drivers (Ceph / NFS / ZFS).">
              <div className="divide-y divide-white/[0.04]">
                {backends.length === 0 && <p className="text-sm text-[var(--text-muted)] py-4">No backends registered.</p>}
                {backends.map((b) => (
                  <div key={b.id} className="flex items-center justify-between py-3">
                    <div>
                      <p className="text-sm text-[var(--text-primary)]">{b.name}</p>
                      <p className="text-xs text-[var(--text-muted)]">
                        {b.backend_type} · {b.mode}
                      </p>
                    </div>
                    <StatePill value={b.status} />
                  </div>
                ))}
              </div>
            </MacGlassPanel>
          )}

          {tab === 'Volumes' && (
            <MacGlassPanel title="Volumes" subtitle="Backend volumes provisioned through Atlas.">
              <div className="divide-y divide-white/[0.04]">
                {volumes.length === 0 && <p className="text-sm text-[var(--text-muted)] py-4">No volumes.</p>}
                {volumes.map((v) => (
                  <div key={v.id} className="flex items-center justify-between py-3 gap-3">
                    <div className="min-w-0">
                      <p className="text-sm text-[var(--text-primary)] truncate">{v.name}</p>
                      <p className="text-xs text-[var(--text-muted)] truncate">
                        {fmtBytes(v.size_bytes)} · {v.backend_native_id ?? v.id}
                      </p>
                    </div>
                    <div className="flex items-center gap-2 shrink-0">
                      <StatePill value={v.state} />
                      <button
                        type="button"
                        className="btn-secondary btn-xs"
                        disabled={busy === v.id}
                        onClick={() =>
                          void run(v.id, () => snapshotAtlasVolume(v.id), `Snapshot of ${v.name} queued`)
                        }
                      >
                        Snapshot
                      </button>
                      <button
                        type="button"
                        className="btn-secondary btn-xs"
                        disabled={busy === v.id}
                        onClick={() =>
                          void run(v.id, () => backupAtlasVolume({ volume_id: v.id }), `Backup of ${v.name} queued`)
                        }
                      >
                        Backup
                      </button>
                      <button
                        type="button"
                        className="btn-secondary btn-xs text-rose-400"
                        disabled={busy === v.id}
                        aria-label="Delete volume"
                        onClick={() => {
                          if (window.confirm(`Delete volume ${v.name}?`))
                            void run(v.id, () => deleteAtlasVolume(v.id), `Volume ${v.name} deleting`)
                        }}
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </MacGlassPanel>
          )}

          {tab === 'Snapshots' && (
            <MacGlassPanel title="Snapshots" subtitle="Point-in-time snapshots of Atlas volumes.">
              <div className="divide-y divide-white/[0.04]">
                {snapshots.length === 0 && <p className="text-sm text-[var(--text-muted)] py-4">No snapshots.</p>}
                {snapshots.map((s) => (
                  <div key={s.id} className="flex items-center justify-between py-3 gap-3">
                    <div className="min-w-0">
                      <p className="text-sm text-[var(--text-primary)] truncate">{s.name}</p>
                      <p className="text-xs text-[var(--text-muted)] truncate">vol {s.volume_id}</p>
                    </div>
                    <div className="flex items-center gap-2 shrink-0">
                      <StatePill value={s.state} />
                      <button
                        type="button"
                        className="btn-secondary btn-xs"
                        disabled={busy === s.id}
                        onClick={() =>
                          void run(s.id, () => restoreAtlasSnapshot(s.id), `Restore from ${s.name} queued`)
                        }
                      >
                        Restore
                      </button>
                      <button
                        type="button"
                        className="btn-secondary btn-xs text-rose-400"
                        disabled={busy === s.id}
                        aria-label="Delete snapshot"
                        onClick={() => {
                          if (window.confirm(`Delete snapshot ${s.name}?`))
                            void run(s.id, () => deleteAtlasSnapshot(s.id), `Snapshot ${s.name} deleting`)
                        }}
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </MacGlassPanel>
          )}

          {tab === 'Backups' && (
            <MacGlassPanel title="Backups" subtitle="Volume backups written to Atlas RGW buckets (S3).">
              <div className="divide-y divide-white/[0.04]">
                {backups.length === 0 && <p className="text-sm text-[var(--text-muted)] py-4">No backups.</p>}
                {backups.map((b) => (
                  <div key={b.id} className="flex items-center justify-between py-3 gap-3">
                    <div className="min-w-0">
                      <p className="text-sm text-[var(--text-primary)] truncate">{b.object_key ?? b.id}</p>
                      <p className="text-xs text-[var(--text-muted)] truncate">
                        vol {b.volume_id} · {b.format ?? 'manifest'}
                      </p>
                    </div>
                    <div className="flex items-center gap-2 shrink-0">
                      <StatePill value={b.state} />
                      <button
                        type="button"
                        className="btn-secondary btn-xs text-rose-400"
                        disabled={busy === b.id}
                        aria-label="Delete backup"
                        onClick={() => {
                          if (window.confirm('Delete this backup?'))
                            void run(b.id, () => deleteAtlasBackup(b.id), 'Backup deleting')
                        }}
                      >
                        <Trash2 className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            </MacGlassPanel>
          )}

          {tab === 'Jobs' && (
            <MacGlassPanel title="Jobs" subtitle="Async Atlas operations (create, snapshot, backup, restore).">
              <div className="divide-y divide-white/[0.04]">
                {jobs.length === 0 && <p className="text-sm text-[var(--text-muted)] py-4">No recent jobs.</p>}
                {jobs.map((j) => {
                  const id = j.id ?? j.job_id ?? '?'
                  return (
                    <div key={id} className="flex items-center justify-between py-3 gap-3">
                      <div className="min-w-0">
                        <p className="text-sm text-[var(--text-primary)] truncate">{j.job_type ?? id}</p>
                        {j.error && <p className="text-xs text-rose-400 truncate">{j.error}</p>}
                      </div>
                      <div className="flex items-center gap-2 shrink-0">
                        <span className="text-xs text-[var(--text-muted)]">{j.progress_percent ?? 0}%</span>
                        <StatePill value={j.state} />
                      </div>
                    </div>
                  )
                })}
              </div>
            </MacGlassPanel>
          )}
        </div>
      )}
    </PlatformPageChrome>
  )
}

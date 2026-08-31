// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Link } from 'react-router'
import { Check, Disc, Download, Plus, RefreshCw, ShieldAlert, ShieldCheck, Upload, X } from 'lucide-react'
import ConfirmDialog from '../../components/ConfirmDialog'
import PlatformEmptyState from '../../components/platform/PlatformEmptyState'
import PlatformFilterPills from '../../components/platform/PlatformFilterPills'
import PlatformPageChrome, { PlatformBackLink, PlatformRefreshButton } from '../../components/platform/PlatformPageChrome'
import { MacSheet, MacStatWidget, gradientForName } from '../../components/platform/mac/PlatformMacUi'
import {
  approveContentImage,
  createContentImage,
  listContentImages,
  rejectContentImage,
  type ContentImage,
} from '../../api/platform'
import { uploadIso, downloadIsoFromUrl, listJobs, type JobSummary } from '../../api/extras'
import { useToastContext } from '../../contexts/ToastContext'
import { formatUserError } from '../../utils/apiError'
import { hubLinkClasses, statusBadgeClasses, statusSurfaceClasses, statusToneClass } from '../../utils/semanticColors'

const CATEGORIES = [
  'Operating Systems',
  'Ubuntu',
  'Debian',
  'Rocky Linux',
  'Windows Server',
  'Custom Appliances',
] as const

function guessCategory(name: string): string {
  const n = name.toLowerCase()
  if (n.includes('ubuntu')) return 'Ubuntu'
  if (n.includes('debian')) return 'Debian'
  if (n.includes('rocky') || n.includes('alma') || n.includes('rhel')) return 'Rocky Linux'
  if (n.includes('windows')) return 'Windows Server'
  return 'Custom Appliances'
}

function lifecycleBadge(status: string) {
  if (status === 'available') return { label: 'Approved', tone: statusToneClass('ok'), icon: ShieldCheck }
  if (status === 'pending') return { label: 'Pending approval', tone: statusToneClass('warn'), icon: ShieldAlert }
  if (status === 'rejected') return { label: 'Rejected', tone: statusToneClass('error'), icon: ShieldAlert }
  return { label: status, tone: statusToneClass('neutral'), icon: ShieldAlert }
}

export default function PlatformContent() {
  const toast = useToastContext()
  const [rows, setRows] = useState<ContentImage[]>([])
  const [error, setError] = useState<string | null>(null)
  const [category, setCategory] = useState<string>('Operating Systems')
  const [filter, setFilter] = useState<'all' | 'pending' | 'available'>('all')
  const [name, setName] = useState('ubuntu-24.04.iso')
  const [path, setPath] = useState('/var/lib/libvirt/images/ubuntu-24.04.iso')
  const [description, setDescription] = useState('')
  const [sheetOpen, setSheetOpen] = useState(false)
  const [rejectTargetId, setRejectTargetId] = useState<string | null>(null)
  const [uploadMode, setUploadMode] = useState<'file' | 'url' | 'path'>('file')
  const [downloadUrl, setDownloadUrl] = useState('')
  const [downloadJobs, setDownloadJobs] = useState<JobSummary[]>([])
  const [uploadFile, setUploadFile] = useState<File | null>(null)
  const [uploadPct, setUploadPct] = useState(0)
  const [uploading, setUploading] = useState(false)
  const uploadAbortRef = useRef<AbortController | null>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)

  const load = useCallback(async () => {
    setError(null)
    try {
      setRows(await listContentImages())
    } catch (e: unknown) {
      setError(formatUserError(e))
    }
  }, [])

  useEffect(() => { void load() }, [load])

  const pending = useMemo(() => rows.filter((r) => r.status === 'pending'), [rows])

  const filtered = useMemo(() => {
    let list = rows
    if (filter === 'pending') list = list.filter((r) => r.status === 'pending')
    if (filter === 'available') list = list.filter((r) => r.status === 'available')
    if (category === 'Operating Systems') return list
    return list.filter((r) => (r.category ?? guessCategory(r.name)) === category)
  }, [rows, category, filter])

  const add = async () => {
    try {
      await createContentImage({
        name,
        kind: 'iso',
        path,
        category: guessCategory(name),
        description: description || undefined,
      })
      toast.success('ISO submitted for approval')
      await load()
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    }
  }

  /**
   * Send the file to the hypervisor, then register the resulting host path in the
   * library. Registration is a separate call, so a successful upload whose
   * registration fails still leaves the ISO on the host — the message says so
   * rather than implying the bytes were lost.
   */
  const uploadAndRegister = async () => {
    if (!uploadFile) return
    const controller = new AbortController()
    uploadAbortRef.current = controller
    setUploading(true)
    setUploadPct(0)
    let uploadedPath = ''
    try {
      const res = await uploadIso(uploadFile, {
        signal: controller.signal,
        onProgress: (pct) => setUploadPct(pct),
      })
      uploadedPath = res.path
      await createContentImage({
        name: res.name,
        kind: 'iso',
        path: res.path,
        category: guessCategory(res.name),
        description: description || undefined,
        size_gib: Math.max(1, Math.round(res.size_bytes / (1024 * 1024 * 1024))),
      })
      toast.success(`${res.name} uploaded and submitted for approval`)
      setUploadFile(null)
      if (fileInputRef.current) fileInputRef.current.value = ''
      setSheetOpen(false)
      await load()
    } catch (e: unknown) {
      toast.error(
        uploadedPath
          ? `Uploaded to ${uploadedPath} but could not add it to the library: ${formatUserError(e)}`
          : formatUserError(e),
      )
    } finally {
      setUploading(false)
      setUploadPct(0)
      uploadAbortRef.current = null
    }
  }

  // Downloads that have already been added to the library, so a poll tick does
  // not register the same finished job over and over.
  const registeredJobsRef = useRef<Set<string>>(new Set())

  /** Poll the job registry while any download is still running. */
  const refreshDownloadJobs = useCallback(async () => {
    try {
      const all = await listJobs()
      const isoJobs = all.filter((j) => j.kind === 'iso_download')
      setDownloadJobs(isoJobs)

      // A finished download lands on the hypervisor but is invisible to the
      // library until it is registered — uploads did this and downloads did not,
      // so a downloaded ISO could not be picked in the Create-from-ISO wizard.
      for (const j of isoJobs) {
        if (j.status !== 'completed' || !j.target_path) continue
        if (registeredJobsRef.current.has(j.id)) continue
        registeredJobsRef.current.add(j.id)
        const isoName = j.target_path.split('/').pop() || j.target_path
        try {
          await createContentImage({
            name: isoName,
            kind: 'iso',
            path: j.target_path,
            category: guessCategory(isoName),
            size_gib: j.bytes_total ? Math.max(1, Math.round(j.bytes_total / (1024 * 1024 * 1024))) : undefined,
          })
          toast.success(`${isoName} added to the library`)
          await load()
        } catch {
          // Most often a duplicate path from a re-download — the ISO is on the
          // host either way, so this must not surface as a scary error.
        }
      }
    } catch {
      // A transient failure here must not blank the list the operator is watching.
    }
  }, [load, toast])

  useEffect(() => {
    void refreshDownloadJobs()
  }, [refreshDownloadJobs])

  useEffect(() => {
    if (!downloadJobs.some((j) => j.status === 'running')) return
    const t = setInterval(() => void refreshDownloadJobs(), 2000)
    return () => clearInterval(t)
  }, [downloadJobs, refreshDownloadJobs])

  const startDownload = async () => {
    const url = downloadUrl.trim()
    if (!url) return
    try {
      const started = await downloadIsoFromUrl({ url, overwrite: true })
      toast.success(`Downloading ${started.name} — track it below`)
      setDownloadUrl('')
      await refreshDownloadJobs()
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    }
  }

  const approve = async (id: string) => {
    try {
      await approveContentImage(id)
      toast.success('ISO approved for production use')
      await load()
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    }
  }

  const reject = async (id: string) => {
    try {
      await rejectContentImage(id)
      toast.success('ISO rejected')
      await load()
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    } finally {
      setRejectTargetId(null)
    }
  }

  return (
    <PlatformPageChrome
      eyebrow="Platform"
      error={error}
      onErrorRetry={() => void load()}
      prepend={<PlatformBackLink to="/platform/infrastructure" label="Infrastructure" />}
      title="Content Library"
      subtitle="ISO grid with approval inbox — upload golden images for templates."
      icon={<Disc className="w-6 h-6 text-[var(--text-muted)]" />}
      actions={
        <>
          {pending.length > 0 && (
            <span className={`px-2 py-1 rounded-full text-xs border ${statusBadgeClasses('warn')}`}>{pending.length} pending</span>
          )}
          <PlatformRefreshButton onClick={() => void load()} />
          <Link to="/platform/create-iso" className="btn-secondary flex items-center gap-2 text-sm">
            <Disc className="w-4 h-4" /> Create from ISO
          </Link>
          <button type="button" className="btn-primary flex items-center gap-2" onClick={() => setSheetOpen(true)}><Plus className="w-4 h-4" /> Upload</button>
        </>
      }
      contentClassName="space-y-4"
    >

      {pending.length > 0 && (
        <section className={`card p-4 ${statusSurfaceClasses('warn')}`}>
          <h2 className={`text-sm font-semibold mb-3 ${statusToneClass('warn')}`}>Approval queue ({pending.length})</h2>
          <div className="space-y-2">
            {pending.map((r) => (
              <div key={r.id} className="flex flex-wrap items-center justify-between gap-3 rounded-xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)] px-4 py-3">
                <div>
                  <p className="font-medium">{r.name}</p>
                  <p className="text-xs text-[var(--text-muted)] font-mono truncate max-w-md">{r.path}</p>
                  {r.submitted_by && <p className="text-[10px] text-[var(--text-faint)] mt-1">Submitted by {r.submitted_by}</p>}
                </div>
                <div className="flex gap-2">
                  <button type="button" className="btn-primary text-xs flex items-center gap-1" onClick={() => void approve(r.id)}>
                    <Check className="w-3 h-3" /> Approve
                  </button>
                  <button type="button" className="btn-danger text-xs flex items-center gap-1" onClick={() => setRejectTargetId(r.id)}>
                    <X className="w-3 h-3" /> Reject
                  </button>
                </div>
              </div>
            ))}
          </div>
        </section>
      )}

      <div className="grid gap-3 sm:grid-cols-3">
        <MacStatWidget label="Images" value={String(rows.length)} icon={<Disc className="w-4 h-4" />} />
        <MacStatWidget label="Pending" value={String(pending.length)} tone={pending.length ? 'warn' : 'ok'} icon={<ShieldAlert className="w-4 h-4" />} />
        <MacStatWidget label="Approved" value={String(rows.filter((r) => r.status === 'available').length)} tone="ok" icon={<ShieldCheck className="w-4 h-4" />} />
      </div>

      <PlatformFilterPills
        value={filter}
        onChange={(id) => setFilter(id as typeof filter)}
        options={[
          { id: 'all', label: 'All' },
          { id: 'pending', label: 'Pending', count: pending.length },
          { id: 'available', label: 'Approved' },
        ]}
      />

      <div className="flex flex-wrap gap-2">
        {CATEGORIES.map((c) => (
          <button
            key={c}
            type="button"
            onClick={() => setCategory(c)}
            className={`px-3 py-1.5 rounded-full text-xs ${category === c ? 'bg-[var(--surface-hover)] text-[var(--text-primary)]' : 'bg-[var(--apple-surface)] text-[var(--text-muted)] border border-[var(--apple-hairline)]'}`}
          >
            {c}
          </button>
        ))}
      </div>

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
        {filtered.map((r) => {
          const badge = lifecycleBadge(r.status)
          const BadgeIcon = badge.icon
          return (
            <article key={r.id} className="platform-mac-stat rounded-2xl border border-white/[0.06] bg-[var(--apple-surface)] p-5">
              <div className="flex items-start gap-3">
                <div className={`w-10 h-10 rounded-xl bg-gradient-to-br ${gradientForName(r.name)} flex items-center justify-center`}>
                  <Disc className="w-5 h-5 text-white" />
                </div>
                <div className="min-w-0 flex-1">
                  <h3 className="font-semibold truncate">{r.name}</h3>
                  <span className={`text-[10px] flex items-center gap-1 ${badge.tone}`}>
                    <BadgeIcon className="w-3 h-3" /> {badge.label}
                  </span>
                </div>
              </div>
              <p className="text-xs text-[var(--text-muted)] mt-1">
                {r.kind.toUpperCase()} · {r.category ?? guessCategory(r.name)}
              </p>
              {r.description && <p className="text-xs text-[var(--text-muted)] mt-2">{r.description}</p>}
              <p className="text-xs font-mono text-[var(--text-faint)] mt-2 truncate">{r.path}</p>
              <p className="text-[10px] text-[var(--text-faint)] mt-3">
                {r.status === 'available'
                  ? 'Approved — safe for production VM creation.'
                  : r.status === 'pending'
                    ? 'Awaiting administrator approval.'
                    : r.rejected_reason ?? 'Not approved for production use.'}
              </p>
              {r.status === 'pending' && (
                <div className="flex gap-2 mt-3">
                  <button type="button" className="btn-primary text-xs" onClick={() => void approve(r.id)}>Approve</button>
                  <button type="button" className="btn-danger text-xs" onClick={() => setRejectTargetId(r.id)}>Reject</button>
                </div>
              )}
              {r.status === 'available' && r.kind === 'iso' && (
                <Link
                  to={`/platform/create-iso?iso_path=${encodeURIComponent(r.path)}`}
                  className={`btn-secondary text-xs mt-3 inline-block text-center w-full ${hubLinkClasses()}`}
                >
                  Create VM from ISO
                </Link>
              )}
            </article>
          )
        })}
      </div>
      {filtered.length === 0 && (
        <PlatformEmptyState title="No images" subtitle="Upload an ISO or qcow2 path for administrator approval." />
      )}

      <MacSheet
        open={sheetOpen}
        onClose={() => { if (!uploading) setSheetOpen(false) }}
        title="Add image"
        subtitle="Upload an ISO from this computer, or register one already on a hypervisor. Approval is required for production."
      >
        <div className="space-y-3">
          <PlatformFilterPills
            options={[
              { id: 'file', label: 'Upload from this computer' },
              { id: 'url', label: 'Download from a URL' },
              { id: 'path', label: 'Register a host path' },
            ]}
            value={uploadMode}
            onChange={(v) => setUploadMode(v as 'file' | 'url' | 'path')}
          />

          {uploadMode === 'file' ? (
            <>
              {/* Not the shared `input` class: that styles a text field, and its
                  placeholder renders underneath the native file-picker button. */}
              <input
                ref={fileInputRef}
                type="file"
                accept=".iso,application/x-cd-image"
                aria-label="ISO file"
                className="w-full rounded-xl border border-white/10 bg-[var(--apple-surface)]/50 p-2 text-sm text-[var(--text-secondary)] file:mr-3 file:cursor-pointer file:rounded-lg file:border-0 file:bg-[var(--accent)]/20 file:px-3 file:py-1.5 file:text-sm file:font-medium file:text-[var(--link)] hover:file:bg-[var(--accent)]/30 disabled:opacity-50"
                disabled={uploading}
                onChange={(e) => setUploadFile(e.target.files?.[0] ?? null)}
              />
              {uploadFile && (
                <p className="text-xs text-[var(--text-muted)]">
                  {uploadFile.name} — {(uploadFile.size / (1024 * 1024 * 1024)).toFixed(2)} GiB
                </p>
              )}
              <input className="input w-full" aria-label="Description" placeholder="description (optional)" value={description} onChange={(e) => setDescription(e.target.value)} disabled={uploading} />
              {uploading && (
                <div className="space-y-1">
                  <div className="h-2 w-full rounded-full bg-white/10 overflow-hidden">
                    <div className="h-full bg-[var(--accent)] transition-all" style={{ width: `${uploadPct}%` }} />
                  </div>
                  <p className="text-xs text-[var(--text-muted)]">Uploading… {uploadPct}% — keep this tab open.</p>
                </div>
              )}
              <div className="flex gap-2">
                <button
                  type="button"
                  className="btn-primary flex-1 flex items-center justify-center gap-2"
                  disabled={!uploadFile || uploading}
                  onClick={() => void uploadAndRegister()}
                >
                  <Upload className="w-4 h-4" /> {uploading ? 'Uploading…' : 'Upload'}
                </button>
                {uploading && (
                  <button type="button" className="btn-secondary" onClick={() => uploadAbortRef.current?.abort()}>
                    Cancel
                  </button>
                )}
              </div>
            </>
          ) : uploadMode === 'url' ? (
            <>
              <input
                className="input w-full font-mono text-xs"
                aria-label="ISO URL"
                placeholder="https://releases.ubuntu.com/…/ubuntu-24.04-live-server-amd64.iso"
                value={downloadUrl}
                onChange={(e) => setDownloadUrl(e.target.value)}
              />
              <p className="text-xs text-[var(--text-muted)]">
                The hypervisor fetches this directly — far faster than uploading from your laptop, and it keeps
                running if you close this tab. Several downloads can run at once.
              </p>
              <button type="button" className="btn-primary w-full flex items-center justify-center gap-2" disabled={!downloadUrl.trim()} onClick={() => void startDownload()}>
                <Download className="w-4 h-4" /> Start download
              </button>

              {downloadJobs.length > 0 && (
                <div className="space-y-2 pt-2 border-t border-white/[0.06]">
                  <p className="text-xs text-[var(--text-muted)]">Downloads</p>
                  {downloadJobs.slice(0, 6).map((j) => {
                    const pct = j.bytes_total && j.bytes_total > 0
                      ? Math.min(100, Math.round(((j.bytes_done ?? 0) / j.bytes_total) * 100))
                      : null
                    return (
                      <div key={j.id} className="space-y-1">
                        <div className="flex items-center justify-between gap-2 text-xs">
                          <span className="truncate text-[var(--text-secondary)]">{j.title.replace('Download ISO: ', '')}</span>
                          <span className={
                            j.status === 'failed' ? 'text-red-600'
                              : j.status === 'completed' ? 'text-emerald-600'
                                : 'text-[var(--text-muted)]'
                          }>
                            {j.status === 'running'
                              ? (pct !== null ? `${pct}%` : `${Math.round((j.bytes_done ?? 0) / (1024 * 1024))} MB`)
                              : j.status}
                          </span>
                        </div>
                        <div className="h-1.5 w-full rounded-full bg-white/10 overflow-hidden">
                          <div
                            className={`h-full transition-all ${j.status === 'failed' ? 'bg-red-500' : j.status === 'completed' ? 'bg-emerald-500' : 'bg-[var(--accent)]'} ${pct === null && j.status === 'running' ? 'animate-pulse' : ''}`}
                            style={{ width: j.status === 'completed' ? '100%' : `${pct ?? 100}%` }}
                          />
                        </div>
                        {j.error ? <p className="text-[11px] text-red-600/90">{j.error}</p> : null}
                      </div>
                    )
                  })}
                </div>
              )}
            </>
          ) : (
            <>
              <input className="input w-full" aria-label="Image name" placeholder="name" value={name} onChange={(e) => setName(e.target.value)} />
              <input className="input w-full" aria-label="Host path" placeholder="host path" value={path} onChange={(e) => setPath(e.target.value)} />
              <input className="input w-full" aria-label="Description" placeholder="description (optional)" value={description} onChange={(e) => setDescription(e.target.value)} />
              <button type="button" className="btn-primary w-full" onClick={async () => { await add(); setSheetOpen(false) }}>Submit for approval</button>
            </>
          )}
        </div>
      </MacSheet>
      <ConfirmDialog
        open={rejectTargetId !== null}
        title="Reject ISO"
        message={`Reject "${rows.find((r) => r.id === rejectTargetId)?.name}"? It will not be available for VM creation.`}
        confirmLabel="Reject"
        variant="danger"
        onCancel={() => setRejectTargetId(null)}
        onConfirm={() => { if (rejectTargetId) void reject(rejectTargetId) }}
      />
    </PlatformPageChrome>
  )
}

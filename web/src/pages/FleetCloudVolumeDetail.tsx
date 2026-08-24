// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router'
import { ArrowLeft, Disc } from 'lucide-react'
import {
  createVolumeSnapshot,
  deleteVolume,
  deleteVolumeSnapshot,
  detachVolume,
  extendVolume,
  getVolume,
  listVolumeSnapshots,
  type NativeVolume,
  type NativeVolumeSnapshot,
} from '../api/nativeVolumes'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import PageSkeleton from '../components/PageSkeleton'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { statusActionLinkClasses } from '../utils/semanticColors'
import { useBreadcrumbName } from '../contexts/BreadcrumbNameContext'

// Native volumes — not gated by <OpenStackGate>. Narrower than Cinder: no
// rename, no bootable flag, no upload-to-image (no native equivalent) — see
// api/nativeVolumes.ts.
export default function OpenStackVolumeDetailPage() {
  return <OpenStackVolumeDetailContent />
}

function OpenStackVolumeDetailContent() {
  const { id } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const toast = useToastContext()
  const [vol, setVol] = useState<NativeVolume | null>(null)
  const [snapshots, setSnapshots] = useState<NativeVolumeSnapshot[]>([])
  const [loading, setLoading] = useState(true)
  useBreadcrumbName(vol?.name)
  const loadSeq = useRef(0)

  const load = useCallback(async () => {
    if (!id) return
    // Last-response-wins: only the newest load may commit so a stale fetch for a
    // prior volume can't overwrite the one now shown.
    const seq = ++loadSeq.current
    const alive = () => seq === loadSeq.current
    setLoading(true)
    try {
      const [v, s] = await Promise.all([getVolume(id), listVolumeSnapshots(id).catch(() => [])])
      if (!alive()) return
      setVol(v)
      setSnapshots(s)
    } catch (e: unknown) {
      if (!alive()) return
      toast.error(formatUserError(e))
      setVol(null)
    } finally {
      if (alive()) setLoading(false)
    }
  }, [id, toast])

  useEffect(() => {
    void load()
  }, [load])

  if (loading) return <PageSkeleton />
  if (!vol) {
    return (
      <div className="space-y-4">
        <FleetCloudSubNav />
        <p className="text-slate-400">Volume not found.</p>
        <Link to="/fleet-cloud/volumes" className="text-sky-400 hover:underline">Back</Link>
      </div>
    )
  }

  return (
    <PageLayout
      hideHeader
      className="max-w-3xl"
      prepend={<><FleetCloudSubNav /></>}
    >
      <Link to="/fleet-cloud/volumes" className="inline-flex items-center gap-2 text-slate-400 hover:text-slate-200 text-sm">
        <ArrowLeft className="w-4 h-4" /> Volumes
      </Link>
      <h1 className="text-2xl font-semibold flex items-center gap-2">
        <Disc className="w-7 h-7 text-sky-400" />
        {vol.name}
      </h1>
      <dl className="grid sm:grid-cols-2 gap-4 rounded-xl border border-slate-700 p-4 text-sm">
        <div><dt className="text-xs text-slate-500 uppercase">ID</dt><dd className="font-mono text-slate-200 mt-1 break-all">{vol.id}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Size</dt><dd className="text-slate-200 mt-1">{vol.size_gib} GiB</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Status</dt><dd className="text-slate-200 mt-1">{vol.status}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Class</dt><dd className="text-slate-200 mt-1">{vol.volume_class}{vol.atlas_backed ? ' (Atlas)' : ''}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Attached</dt><dd className="mt-1 font-mono text-xs">
          {vol.attached_vm_id ? (
            <Link to={`/fleet-cloud/instances/${vol.attached_vm_id}`} className="text-sky-400 hover:underline">{vol.attached_vm_id}</Link>
          ) : '—'}
        </dd></div>
        {vol.attached_device && <div><dt className="text-xs text-slate-500 uppercase">Device</dt><dd className="font-mono text-slate-200 mt-1">{vol.attached_device}</dd></div>}
      </dl>
      <div className="flex flex-wrap gap-2">
        {vol.attached_vm_id && (
          <button type="button" className={`px-3 py-1.5 rounded-lg border text-sm ${statusActionLinkClasses('warn')}`}
            onClick={async () => {
              try {
                await detachVolume(vol.id)
                toast.success('Detached')
                void load()
              } catch (e: unknown) { toast.error(formatUserError(e)) }
            }}>Detach</button>
        )}
        <button type="button" className="px-3 py-1.5 rounded-lg border border-slate-600 text-sm"
          onClick={async () => {
            const n = prompt('New size (GiB)', String(vol.size_gib + 1))
            if (!n) return
            const size = Number.parseInt(n, 10)
            if (!Number.isFinite(size) || size <= vol.size_gib) {
              toast.warning(`Enter a whole number of GiB greater than ${vol.size_gib}`)
              return
            }
            try {
              await extendVolume(vol.id, size)
              toast.success('Extended')
              void load()
            } catch (e: unknown) { toast.error(formatUserError(e)) }
          }}>Extend</button>
        <button type="button" className={`px-3 py-1.5 rounded-lg border text-sm ${statusActionLinkClasses('error')}`}
          onClick={async () => {
            if (!confirm(`Delete volume ${vol.name}?`)) return
            try {
              await deleteVolume(vol.id)
              toast.success('Deleted')
              navigate('/fleet-cloud/volumes')
            } catch (e: unknown) { toast.error(formatUserError(e)) }
          }}>Delete</button>
      </div>

      <section className="rounded-xl border border-slate-700 p-4 space-y-3">
        <div className="flex items-center justify-between">
          <h2 className="text-sm font-medium text-slate-300">Snapshots</h2>
          <button type="button" className="text-xs text-sky-400 hover:underline"
            onClick={async () => {
              const n = prompt('Snapshot name', `${vol.name}-snap`)
              if (!n) return
              try {
                await createVolumeSnapshot(vol.id, n)
                toast.success('Snapshot requested')
                void load()
              } catch (e: unknown) { toast.error(formatUserError(e)) }
            }}>+ Snapshot</button>
        </div>
        {snapshots.length === 0 ? (
          <p className="text-sm text-slate-500">No snapshots.</p>
        ) : (
          <ul className="text-sm font-mono space-y-1">
            {snapshots.map((s) => (
              <li key={s.id} className="flex items-center gap-2">
                <span>{s.name}</span>
                <span className="text-slate-500 text-xs">{s.status}</span>
                <button type="button" className={statusActionLinkClasses('error', 'text-xs ml-auto')}
                  onClick={async () => {
                    if (!confirm(`Delete snapshot ${s.name}?`)) return
                    try {
                      await deleteVolumeSnapshot(s.id)
                      toast.success('Deleted')
                      void load()
                    } catch (e: unknown) { toast.error(formatUserError(e)) }
                  }}>Delete</button>
              </li>
            ))}
          </ul>
        )}
      </section>
      <FleetCloudFooter />
    </PageLayout>
  )
}

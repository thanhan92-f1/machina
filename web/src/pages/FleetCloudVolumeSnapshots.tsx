// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router'
import {
  deleteVolumeSnapshot,
  listAllVolumeSnapshots,
  type NativeVolumeSnapshotWithVolume,
} from '../api/nativeVolumes'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { statusActionLinkClasses } from '../utils/semanticColors'
import { Camera, Loader2, RefreshCw } from 'lucide-react'

// Native volume snapshots (controller::api::volumes) -- like the other rewired
// /fleet-cloud/* pages, this no longer depends on a wired external cloud.
// There's no old external-cloud gate component wrapping it any more either:
// the daemon's external-cloud-client integration has since been fully
// removed. Snapshots require an Atlas-backed volume (ATLAS_ENABLED=1) -- see
// api/nativeVolumes.ts.
export default function FleetCloudVolumeSnapshotsPage() {
  return <FleetCloudVolumeSnapshotsContent />
}

function FleetCloudVolumeSnapshotsContent() {
  const toast = useToastContext()
  const [snapshots, setSnapshots] = useState<NativeVolumeSnapshotWithVolume[]>([])
  const [loading, setLoading] = useState(true)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const list = await listAllVolumeSnapshots()
      setSnapshots(list)
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => { void load() }, [load])

  return (
    <PageLayout
      hideHeader
      className="w-full max-w-none"
      prepend={<><FleetCloudSubNav /></>}
    >
      <div className="flex items-center justify-between gap-3">
        <p className="apple-eyebrow">Fleet Cloud</p>
      <h1 className="page-title flex items-center gap-3">
          <Camera className="w-7 h-7 text-[var(--accent)]" />
          Storage volume snapshots
        </h1>
        <button type="button" onClick={() => void load()} className="btn-secondary text-sm inline-flex items-center gap-1">
          <RefreshCw className="w-4 h-4" /> Refresh
        </button>
      </div>
      {loading ? (
        <Loader2 className="w-8 h-8 animate-spin text-[var(--accent)] mx-auto" />
      ) : (
        <div className="rounded-2xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)] overflow-hidden">
          <table className="apple-table" aria-label="Volume snapshots">
            <thead>
              <tr>
                <th scope="col" className="px-3 py-2">Name</th>
                <th scope="col" className="px-3 py-2">Volume</th>
                <th scope="col" className="px-3 py-2">Status</th>
                <th scope="col" className="px-3 py-2">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-[var(--apple-hairline)] font-mono text-xs">
              {snapshots.map((s) => (
                <tr key={s.id}>
                  <td className="px-3 py-2 text-[var(--text-primary)]">
                    <Link to={`/fleet-cloud/volume-snapshots/${s.id}`} className="text-[var(--link)] hover:underline">{s.name || s.id.slice(0, 8)}</Link>
                  </td>
                  <td className="px-3 py-2">
                    <Link to={`/fleet-cloud/volumes/${s.volume_id}`} className="text-[var(--accent)] hover:underline">{s.volume_name}</Link>
                  </td>
                  <td className="px-3 py-2 text-[var(--text-muted)]">{s.status}</td>
                  <td className="px-3 py-2 flex flex-wrap gap-2">
                    <Link to={`/fleet-cloud/volume-snapshots/${s.id}`} className="text-[var(--accent)] hover:underline">Detail</Link>
                    <button type="button" className={statusActionLinkClasses('error')} onClick={async () => {
                      if (!confirm(`Delete snapshot ${s.name || s.id}?`)) return
                      try {
                        await deleteVolumeSnapshot(s.id)
                        toast.success('Deleted')
                        void load()
                      } catch (e: unknown) { toast.error(formatUserError(e)) }
                    }}>Delete</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {snapshots.length === 0 && <p className="p-6 text-center text-[var(--text-muted)]">No snapshots.</p>}
        </div>
      )}
      <FleetCloudFooter />
    </PageLayout>
  )
}

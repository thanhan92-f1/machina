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
// /fleet-cloud/* pages, this no longer depends on a wired external OpenStack
// cloud. There's no <OpenStackGate> component to wrap it in any more either:
// the daemon's external-OpenStack-client integration has since been fully
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
      className="max-w-4xl"
      prepend={<><FleetCloudSubNav /></>}
    >
      <div className="flex items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold flex items-center gap-2">
          <Camera className="w-7 h-7 text-sky-400" />
          Storage volume snapshots
        </h1>
        <button type="button" onClick={() => void load()} className="inline-flex items-center gap-1 px-3 py-1.5 rounded-lg border border-slate-600 text-sm">
          <RefreshCw className="w-4 h-4" /> Refresh
        </button>
      </div>
      {loading ? (
        <Loader2 className="w-8 h-8 animate-spin text-sky-400 mx-auto" />
      ) : (
        <div className="rounded-xl border border-slate-700 overflow-hidden">
          <table className="w-full text-sm" aria-label="Volume snapshots">
            <thead className="bg-slate-900/80 text-slate-400 text-left">
              <tr>
                <th scope="col" className="px-3 py-2">Name</th>
                <th scope="col" className="px-3 py-2">Volume</th>
                <th scope="col" className="px-3 py-2">Status</th>
                <th scope="col" className="px-3 py-2">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800 font-mono text-xs">
              {snapshots.map((s) => (
                <tr key={s.id}>
                  <td className="px-3 py-2 text-slate-200">
                    <Link to={`/fleet-cloud/volume-snapshots/${s.id}`} className="text-sky-300 hover:underline">{s.name || s.id.slice(0, 8)}</Link>
                  </td>
                  <td className="px-3 py-2">
                    <Link to={`/fleet-cloud/volumes/${s.volume_id}`} className="text-sky-400 hover:underline">{s.volume_name}</Link>
                  </td>
                  <td className="px-3 py-2 text-slate-400">{s.status}</td>
                  <td className="px-3 py-2 flex flex-wrap gap-2">
                    <Link to={`/fleet-cloud/volume-snapshots/${s.id}`} className="text-violet-400 hover:underline">Detail</Link>
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
          {snapshots.length === 0 && <p className="p-6 text-center text-slate-500">No snapshots.</p>}
        </div>
      )}
      <FleetCloudFooter />
    </PageLayout>
  )
}

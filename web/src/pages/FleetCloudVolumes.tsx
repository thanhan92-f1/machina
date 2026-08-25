// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router'
import {
  attachVolume,
  createVolume,
  createVolumeSnapshot,
  deleteVolume,
  deleteVolumeSnapshot,
  detachVolume,
  extendVolume,
  listVolumeSnapshots,
  listVolumes,
  type NativeVolume,
  type NativeVolumeSnapshot,
} from '../api/nativeVolumes'
import { listVms, type NativeVm } from '../api/nativeVms'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import PageSkeleton from '../components/PageSkeleton'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { statusActionLinkClasses } from '../utils/semanticColors'
import { HardDrive, RefreshCw } from 'lucide-react'

// Native standalone volumes — no old external-cloud gate component in the
// way any more (the daemon's external-cloud-client integration has
// since been fully removed). Narrower than Cinder: no
// transfers/retype/clone/bootable-flag/create-from-image (no native
// equivalent yet) — see api/nativeVolumes.ts.
export default function FleetCloudVolumesPage() {
  return <FleetCloudVolumesContent />
}

function FleetCloudVolumesContent() {
  const toast = useToastContext()
  const [volumes, setVolumes] = useState<NativeVolume[]>([])
  const [snapshotsByVol, setSnapshotsByVol] = useState<Record<string, NativeVolumeSnapshot[]>>({})
  const [loading, setLoading] = useState(true)
  const [sizeGb, setSizeGb] = useState('10')
  const [name, setName] = useState('')
  const [creating, setCreating] = useState(false)
  const [attachVolId, setAttachVolId] = useState('')
  const [attachInstId, setAttachInstId] = useState('')
  const [instances, setInstances] = useState<NativeVm[]>([])

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const [v, inst] = await Promise.all([listVolumes(), listVms().catch(() => [])])
      setVolumes(v)
      setInstances(inst)
      if (!attachInstId && inst.length > 0) setAttachInstId(inst[0].id)
      const free = v.filter((vol) => !vol.attached_vm_id)
      if (!attachVolId && free.length > 0) setAttachVolId(free[0].id)
      const snaps = await Promise.all(v.map((vol) => listVolumeSnapshots(vol.id).catch(() => [])))
      const byVol: Record<string, NativeVolumeSnapshot[]> = {}
      v.forEach((vol, i) => { byVol[vol.id] = snaps[i] })
      setSnapshotsByVol(byVol)
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

  const allSnapshots = Object.values(snapshotsByVol).flat()

  const handleCreate = async () => {
    if (creating) return
    const size = Number.parseInt(sizeGb, 10)
    if (!Number.isFinite(size) || size < 1) {
      toast.warning('Enter valid size in GB')
      return
    }
    setCreating(true)
    try {
      await createVolume({ size_gib: size, name: name.trim() || `vol-${Date.now()}` })
      toast.success('Volume created')
      setName('')
      void load()
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    } finally {
      setCreating(false)
    }
  }

  return (
    <PageLayout
      hideHeader
      className="max-w-4xl"
      prepend={<><FleetCloudSubNav /></>}
    >
      <h1 className="text-2xl font-semibold flex items-center gap-2">
        <HardDrive className="w-7 h-7 text-sky-400" />
        Volumes
      </h1>
      <div className="rounded-xl border border-slate-700 p-4 flex flex-wrap gap-3 items-end">
        <div>
          <label className="block text-xs text-slate-500 mb-1">Size (GB)</label>
          <input type="number" min={1} value={sizeGb} onChange={(e) => setSizeGb(e.target.value)}
            aria-label="Size in GB"
            className="w-24 px-2 py-1.5 rounded-lg bg-slate-900 border border-slate-700 text-sm" />
        </div>
        <div>
          <label className="block text-xs text-slate-500 mb-1">Name</label>
          <input value={name} onChange={(e) => setName(e.target.value)}
            aria-label="Volume name"
            className="px-2 py-1.5 rounded-lg bg-slate-900 border border-slate-700 text-sm" />
        </div>
        <button type="button" onClick={() => void handleCreate()} disabled={creating}
          className="px-3 py-1.5 rounded-lg bg-sky-600 hover:bg-sky-500 disabled:opacity-40 disabled:cursor-not-allowed text-sm text-white">
          {creating ? 'Creating…' : 'Create volume'}
        </button>
        <button type="button" onClick={() => void load()}
          className="ml-auto inline-flex items-center gap-1 px-3 py-1.5 rounded-lg border border-slate-600 text-sm">
          <RefreshCw className="w-4 h-4" /> Refresh
        </button>
      </div>

      <div className="rounded-xl border border-slate-700 p-4 space-y-3">
        <h2 className="text-sm font-medium text-slate-300">Attach volume to instance</h2>
        <div className="flex flex-wrap gap-3 items-end">
          <div className="min-w-[14rem]">
            <label className="block text-xs text-slate-500 mb-1">Volume</label>
            <select
              value={attachVolId}
              onChange={(e) => setAttachVolId(e.target.value)}
              aria-label="Volume to attach"
              className="w-full px-2 py-1.5 rounded-lg bg-slate-900 border border-slate-700 text-sm"
            >
              <option value="">Select volume…</option>
              {volumes.filter((vol) => !vol.attached_vm_id).map((vol) => (
                <option key={vol.id} value={vol.id}>
                  {vol.name} ({vol.size_gib} GiB)
                </option>
              ))}
            </select>
          </div>
          <div className="min-w-[14rem]">
            <label className="block text-xs text-slate-500 mb-1">Instance</label>
            <select
              value={attachInstId}
              onChange={(e) => setAttachInstId(e.target.value)}
              aria-label="Instance to attach to"
              className="w-full px-2 py-1.5 rounded-lg bg-slate-900 border border-slate-700 text-sm"
            >
              <option value="">Select instance…</option>
              {instances.map((i) => (
                <option key={i.id} value={i.id}>{i.name}</option>
              ))}
            </select>
          </div>
          <button
            type="button"
            disabled={!attachVolId || !attachInstId}
            onClick={async () => {
              try {
                await attachVolume(attachVolId, { vm_id: attachInstId })
                toast.success('Volume attached')
                void load()
              } catch (e: unknown) {
                toast.error(formatUserError(e))
              }
            }}
            className="px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-sm text-white disabled:opacity-40"
          >
            Attach
          </button>
        </div>
      </div>

      {loading ? (
        <PageSkeleton />
      ) : (
        <>
          <div className="rounded-xl border border-slate-700 overflow-hidden">
            <div className="px-3 py-2 bg-slate-900 text-xs text-slate-500 uppercase">Volumes</div>
            <div className="overflow-x-auto">
            <table className="w-full text-sm" aria-label="Volumes">
              <thead className="bg-slate-900/80 text-slate-400 text-left">
                <tr>
                  <th scope="col" className="px-3 py-2">Name</th>
                  <th scope="col" className="px-3 py-2">Size</th>
                  <th scope="col" className="px-3 py-2">Status</th>
                  <th scope="col" className="px-3 py-2">Attached</th>
                  <th scope="col" className="px-3 py-2">Actions</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800">
                {volumes.map((v) => (
                  <tr key={v.id}>
                    <td className="px-3 py-2 font-mono text-slate-200">
                      <Link to={`/fleet-cloud/volumes/${v.id}`} className="hover:text-sky-300 hover:underline">
                        {v.name}
                      </Link>
                    </td>
                    <td className="px-3 py-2">{v.size_gib} GiB</td>
                    <td className="px-3 py-2 text-slate-400">{v.status}</td>
                    <td className="px-3 py-2 text-slate-500 font-mono text-xs">
                      {v.attached_vm_id ? v.attached_vm_id.slice(0, 8) : '—'}
                    </td>
                    <td className="px-3 py-2 flex flex-wrap gap-2">
                      {v.attached_vm_id && (
                        <button
                          type="button"
                          className={statusActionLinkClasses('warn', 'text-xs')}
                          onClick={async () => {
                            try {
                              await detachVolume(v.id)
                              toast.success('Volume detached')
                              void load()
                            } catch (e: unknown) {
                              toast.error(formatUserError(e))
                            }
                          }}
                        >
                          Detach
                        </button>
                      )}
                      <button type="button" className="text-xs text-sky-400 hover:underline"
                        onClick={async () => {
                          const n = prompt('Snapshot name', `${v.name}-snap`)
                          if (!n) return
                          try {
                            await createVolumeSnapshot(v.id, n)
                            toast.success('Snapshot requested')
                            void load()
                          } catch (e: unknown) {
                            toast.error(formatUserError(e))
                          }
                        }}>Snapshot</button>
                      <button type="button" className={statusActionLinkClasses('warn', 'text-xs')}
                        onClick={async () => {
                          const n = prompt('New size (GiB)', String(v.size_gib + 1))
                          if (!n) return
                          const size = Number.parseInt(n, 10)
                          if (!Number.isFinite(size) || size <= v.size_gib) {
                            toast.warning(`Enter a whole number of GiB greater than ${v.size_gib}`)
                            return
                          }
                          try {
                            await extendVolume(v.id, size)
                            toast.success('Extended')
                            void load()
                          } catch (e: unknown) {
                            toast.error(formatUserError(e))
                          }
                        }}>Extend</button>
                      <button type="button" className={statusActionLinkClasses('error', 'text-xs')}
                        onClick={async () => {
                          if (!confirm(`Delete volume ${v.name}?`)) return
                          try {
                            await deleteVolume(v.id)
                            toast.success('Deleted')
                            void load()
                          } catch (e: unknown) {
                            toast.error(formatUserError(e))
                          }
                        }}>Delete</button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            </div>
            {volumes.length === 0 && (
              <p className="p-6 text-center text-slate-500 text-sm">No volumes in this project.</p>
            )}
          </div>

          {allSnapshots.length > 0 && (
            <div className="rounded-xl border border-slate-700 overflow-hidden">
              <div className="px-3 py-2 bg-slate-900 text-xs text-slate-500 uppercase flex justify-between">
                <span>Snapshots</span>
                <Link to="/fleet-cloud/volume-snapshots" className="text-sky-400 hover:underline normal-case">View all</Link>
              </div>
              <div className="overflow-x-auto">
              <table className="w-full text-sm" aria-label="Volume snapshots">
                <thead className="bg-slate-900/80 text-slate-400 text-left">
                  <tr>
                    <th scope="col" className="px-3 py-2">Name</th>
                    <th scope="col" className="px-3 py-2">Volume</th>
                    <th scope="col" className="px-3 py-2">Status</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-800 font-mono text-xs">
                  {allSnapshots.map((s) => (
                    <tr key={s.id}>
                      <td className="px-3 py-2 text-slate-200">{s.name}</td>
                      <td className="px-3 py-2 text-slate-500">{s.volume_id.slice(0, 8)}</td>
                      <td className="px-3 py-2">
                        <span className="text-slate-400">{s.status}</span>
                        <button
                          type="button"
                          className={statusActionLinkClasses('error', 'ml-2')}
                          onClick={async () => {
                            if (!confirm(`Delete snapshot ${s.name}?`)) return
                            try {
                              await deleteVolumeSnapshot(s.id)
                              toast.success('Snapshot deleted')
                              void load()
                            } catch (e: unknown) {
                              toast.error(formatUserError(e))
                            }
                          }}
                        >
                          Del
                        </button>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              </div>
            </div>
          )}
        </>
      )}
      <FleetCloudFooter />
    </PageLayout>
  )
}

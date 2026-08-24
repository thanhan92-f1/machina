// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router'
import { ArrowLeft, Cloud, Play, RotateCcw, Square, Trash2 } from 'lucide-react'
import {
  deleteVm,
  getVm,
  listVmDisks,
  listVmNics,
  rebootVm,
  startVm,
  stopVm,
  vmDisplayStatus,
  type NativeVm,
  type NativeVmDisk,
  type NativeVmNic,
} from '../api/nativeVms'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import PageSkeleton from '../components/PageSkeleton'
import ConfirmDialog from '../components/ConfirmDialog'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { openstackStatusTone, statusBadgeClasses, statusActionLinkClasses } from '../utils/semanticColors'
import { useBreadcrumbName } from '../contexts/BreadcrumbNameContext'

// Native VM detail used as the "instance" detail page — not gated by
// <OpenStackGate>. Narrower than the Nova instance detail: no rescue/shelve/
// lock/migrate/backup/resize actions (no native equivalent yet) — start/stop/
// reboot/delete plus disk and NIC inventory, which do have direct native
// equivalents (api::vms).
export default function OpenStackInstanceDetailPage() {
  return <OpenStackInstanceDetailContent />
}

function OpenStackInstanceDetailContent() {
  const { id } = useParams<{ id: string }>()
  const toast = useToastContext()
  const navigate = useNavigate()
  const [vm, setVm] = useState<NativeVm | null>(null)
  const [disks, setDisks] = useState<NativeVmDisk[]>([])
  const [nics, setNics] = useState<NativeVmNic[]>([])
  const [loading, setLoading] = useState(true)
  const [deleting, setDeleting] = useState(false)
  const [confirmDelete, setConfirmDelete] = useState(false)
  useBreadcrumbName(vm?.name)
  const loadSeq = useRef(0)

  const load = useCallback(async () => {
    if (!id) return
    // Last-response-wins: only the newest load may commit so a stale fetch for a
    // prior instance can't overwrite the one now shown.
    const seq = ++loadSeq.current
    const alive = () => seq === loadSeq.current
    setLoading(true)
    try {
      const [v, d, n] = await Promise.all([
        getVm(id),
        listVmDisks(id).catch(() => []),
        listVmNics(id).catch(() => []),
      ])
      if (!alive()) return
      setVm(v)
      setDisks(d)
      setNics(n)
    } catch (e: unknown) {
      if (!alive()) return
      toast.error(formatUserError(e))
      setVm(null)
    } finally {
      if (alive()) setLoading(false)
    }
  }, [id, toast])

  useEffect(() => { void load() }, [load])

  if (loading) return <PageSkeleton />
  if (!vm) {
    return (
      <div className="space-y-4">
        <FleetCloudSubNav />
        <Link to="/fleet-cloud/instances" className="text-sky-400 hover:underline">Back</Link>
      </div>
    )
  }

  const status = vmDisplayStatus(vm)
  const runAction = async (fn: (id: string) => Promise<unknown>, label: string) => {
    try {
      await fn(vm.id)
      toast.success(`${label} queued`)
      void load()
    } catch (e: unknown) {
      toast.error(`${label} failed: ${formatUserError(e)}`)
    }
  }

  return (
    <PageLayout
      hideHeader
      className="max-w-4xl"
      prepend={<><FleetCloudSubNav /></>}
    >
      <Link to="/fleet-cloud/instances" className="inline-flex items-center gap-2 text-slate-400 hover:text-slate-200 text-sm">
        <ArrowLeft className="w-4 h-4" /> Instances
      </Link>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-semibold flex items-center gap-2">
          <Cloud className="w-7 h-7 text-sky-400" /> {vm.name}
        </h1>
        <div className="flex gap-2">
          <button type="button" title="Start" onClick={() => void runAction(startVm, 'Start')}
            className="p-2 rounded border border-slate-600 hover:bg-slate-800"><Play className="w-4 h-4" /></button>
          <button type="button" title="Stop" onClick={() => void runAction(stopVm, 'Stop')}
            className="p-2 rounded border border-slate-600 hover:bg-slate-800"><Square className="w-4 h-4" /></button>
          <button type="button" title="Reboot" onClick={() => void runAction(rebootVm, 'Reboot')}
            className="p-2 rounded border border-slate-600 hover:bg-slate-800"><RotateCcw className="w-4 h-4" /></button>
          <button type="button" title="Delete" onClick={() => setConfirmDelete(true)}
            className={`p-2 rounded border border-red-600/50 ${statusActionLinkClasses('error')}`}><Trash2 className="w-4 h-4" /></button>
        </div>
      </div>

      <dl className="grid sm:grid-cols-2 gap-4 rounded-xl border border-slate-700 p-4 text-sm">
        <div><dt className="text-xs text-slate-500 uppercase">ID</dt><dd className="font-mono mt-1 break-all">{vm.id}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Status</dt><dd className="mt-1">
          <span className={`inline-block px-2 py-0.5 rounded border text-xs ${statusBadgeClasses(openstackStatusTone(status))}`}>{status}</span>
        </dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">vCPU</dt><dd className="mt-1">{vm.vcpus}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">RAM</dt><dd className="mt-1">{vm.memory_mib} MiB</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Project</dt><dd className="mt-1">{vm.project || '—'}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Guest IP</dt><dd className="mt-1 font-mono">{vm.guest_ip || '—'}</dd></div>
        {vm.last_error && (
          <div className="sm:col-span-2"><dt className="text-xs text-slate-500 uppercase">Last error</dt><dd className="mt-1 text-red-300">{vm.last_error}</dd></div>
        )}
      </dl>

      <section className="rounded-xl border border-slate-700 p-4">
        <h2 className="text-sm font-medium text-slate-300 mb-3">Network interfaces</h2>
        {nics.length === 0 ? (
          <p className="text-sm text-slate-500">No NICs.</p>
        ) : (
          <table className="w-full text-sm" aria-label="Network interfaces">
            <thead className="text-slate-400 text-left">
              <tr><th scope="col" className="py-1">Network</th><th scope="col" className="py-1">MAC</th><th scope="col" className="py-1">IP</th></tr>
            </thead>
            <tbody className="font-mono text-xs">
              {nics.map((n) => (
                <tr key={n.mac_address} className="border-t border-slate-800">
                  <td className="py-2">{n.network}</td>
                  <td className="py-2">{n.mac_address}</td>
                  <td className="py-2">{n.ip || '—'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      <section className="rounded-xl border border-slate-700 p-4">
        <h2 className="text-sm font-medium text-slate-300 mb-3">Disks</h2>
        {disks.length === 0 ? (
          <p className="text-sm text-slate-500">No disks.</p>
        ) : (
          <table className="w-full text-sm" aria-label="Disks">
            <thead className="text-slate-400 text-left">
              <tr><th scope="col" className="py-1">Name</th><th scope="col" className="py-1">Size</th><th scope="col" className="py-1">Class</th></tr>
            </thead>
            <tbody className="font-mono text-xs">
              {disks.map((d) => (
                <tr key={d.id} className="border-t border-slate-800">
                  <td className="py-2">{d.name}</td>
                  <td className="py-2">{d.size_gib} GiB</td>
                  <td className="py-2">{d.storage_class}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      <FleetCloudFooter />
      <ConfirmDialog
        open={confirmDelete}
        variant="danger"
        title="Delete instance"
        message={`Delete instance '${vm.name}'? This cannot be undone.`}
        confirmLabel={deleting ? 'Deleting…' : 'Delete'}
        onCancel={() => setConfirmDelete(false)}
        onConfirm={async () => {
          setDeleting(true)
          try {
            await deleteVm(vm.id)
            toast.success('Delete queued')
            navigate('/fleet-cloud/instances')
          } catch (e: unknown) {
            toast.error(formatUserError(e))
          } finally {
            setDeleting(false)
            setConfirmDelete(false)
          }
        }}
      />
    </PageLayout>
  )
}

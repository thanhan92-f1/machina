// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router'
import { listVms, rebootVm, startVm, stopVm, vmDisplayStatus, type NativeVm } from '../api/nativeVms'
import { useToastContext } from '../contexts/ToastContext'
import { Play, Square, RotateCcw, Search, RefreshCw, Cloud, Plus, X } from 'lucide-react'
import FleetCloudFooter from '../components/FleetCloudFooter'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import EmptyState from '../components/EmptyState'
import PageLayout from '../components/PageLayout'
import ConfirmDialog from '../components/ConfirmDialog'
import { formatUserError } from '../utils/apiError'
import { instanceStatusTone, statusBadgeClasses, statusToneClass } from '../utils/semanticColors'

const STATUS_CHIPS = ['', 'ACTIVE', 'SHUTOFF', 'ERROR', 'CREATING'] as const

function statusBadge(status: string) {
  return statusBadgeClasses(instanceStatusTone(status))
}

// Native VM lifecycle used as the "instance" list — not gated by <OpenStackGate>:
// this feature is libvirt-native and does not depend on a wired external
// OpenStack cloud. Unlike the Nova instance list, there's no server-side
// pagination/marker here — the native list endpoint returns the whole project's
// VMs and this page filters client-side.
export default function OpenStackInstancesPage() {
  return <OpenStackInstancesContent />
}

function OpenStackInstancesContent() {
  const [vms, setVms] = useState<NativeVm[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState('')
  const toast = useToastContext()

  const load = useCallback(async () => {
    try {
      setLoadError(null)
      const list = await listVms()
      setVms(list)
    } catch (e: unknown) {
      const msg = formatUserError(e)
      setLoadError(msg)
      toast.error(`Failed to load instances: ${msg}`)
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => { setLoading(true); void load() }, [load])

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase()
    return vms.filter((vm) => {
      if (q && !vm.name.toLowerCase().includes(q) && !vm.id.toLowerCase().includes(q)) return false
      if (statusFilter && vmDisplayStatus(vm) !== statusFilter) return false
      return true
    })
  }, [vms, search, statusFilter])

  const [pendingAction, setPendingAction] = useState<{
    vm: NativeVm
    fn: (id: string) => Promise<unknown>
    label: string
    message: string
  } | null>(null)

  const runAction = async (vm: NativeVm, fn: (id: string) => Promise<unknown>, label: string) => {
    try {
      await fn(vm.id)
      toast.success(`${label} '${vm.name}' queued`)
      void load()
    } catch (e: unknown) {
      toast.error(`${label} failed: ${formatUserError(e)}`)
    }
  }

  return (
    <PageLayout
      prepend={<><FleetCloudSubNav /></>}
      title="Fleet Cloud Instances"
      subtitle={`${vms.length} instance${vms.length === 1 ? '' : 's'}`}
      icon={<Cloud className="w-7 h-7 text-sky-400" />}
      error={loadError}
      errorTitle="Failed to load instances"
      technicalDetail={loadError}
      errorTone="red"
      onErrorRetry={() => void load()}
      onErrorDismiss={() => setLoadError(null)}
      actions={
        <div className="flex gap-2">
          <Link
            to="/fleet-cloud/images"
            className="inline-flex items-center gap-2 px-3 py-2 rounded-lg border border-slate-600 text-slate-200 hover:bg-slate-800 text-sm"
          >
            Images
          </Link>
          <Link
            to="/fleet-cloud/create"
            className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-sky-600 hover:bg-sky-500 text-white text-sm font-medium"
          >
            <Plus className="w-4 h-4" />
            Create instance
          </Link>
          <button
            type="button"
            onClick={() => { setLoading(true); void load() }}
            className="inline-flex items-center gap-2 px-3 py-2 rounded-lg border border-slate-600 text-slate-200 hover:bg-slate-800 text-sm"
          >
            <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
            Refresh
          </button>
        </div>
      }
    >
      <div className="flex flex-wrap gap-3 items-center">
        <div className="relative flex-1 min-w-[200px] max-w-md">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-500" />
          <input
            type="search"
            aria-label="Search instances"
            placeholder="Search name or ID…"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className={`w-full pl-10 py-2 rounded-lg bg-slate-900 border border-slate-700 text-slate-100 text-sm ${search ? 'pr-8' : 'pr-3'}`}
          />
          {search && (
            <button type="button" aria-label="Clear search" onClick={() => setSearch('')}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-200">
              <X className="w-4 h-4" />
            </button>
          )}
        </div>
        <div className="flex flex-wrap gap-1">
          {STATUS_CHIPS.map((chip) => (
            <button
              key={chip || 'all'}
              type="button"
              onClick={() => setStatusFilter(chip)}
              className={`px-3 py-1 rounded-full text-xs font-medium border transition-colors ${
                statusFilter === chip
                  ? 'bg-sky-600 border-sky-500 text-white'
                  : 'border-slate-600 text-slate-400 hover:border-slate-500'
              }`}
            >
              {chip || 'All'}
            </button>
          ))}
        </div>
      </div>

      {!loading && vms.length === 0 ? (
        <EmptyState
          icon={<Cloud className="w-6 h-6" />}
          title="No instances"
          description="No VMs in this project yet."
          secondaryAction={
            <Link to="/fleet-cloud/create" className="px-4 py-2 rounded-lg border border-slate-600 text-slate-300 hover:bg-slate-800 text-sm">
              Create instance
            </Link>
          }
        />
      ) : (
      <div className="overflow-x-auto rounded-xl border border-slate-700/80">
        <table className="w-full text-sm" aria-label="Fleet Cloud instances">
          <thead className="bg-slate-900/80 text-slate-400 text-left">
            <tr>
              <th scope="col" className="px-4 py-3 font-medium">Name</th>
              <th scope="col" className="px-4 py-3 font-medium">Status</th>
              <th scope="col" className="px-4 py-3 font-medium">vCPU / RAM</th>
              <th scope="col" className="px-4 py-3 font-medium">IP</th>
              <th scope="col" className="px-4 py-3 font-medium text-right">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800">
            {loading && filtered.length === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-8 text-center text-slate-500">
                  Loading…
                </td>
              </tr>
            )}
            {!loading && filtered.length === 0 && (
              <tr>
                <td colSpan={5} className="px-4 py-8 text-center text-slate-500">
                  No instances match your filters.
                </td>
              </tr>
            )}
            {filtered.map((vm) => {
              const status = vmDisplayStatus(vm)
              return (
                <tr key={vm.id} className="hover:bg-slate-800/40">
                  <td className="px-4 py-3">
                    <Link
                      to={`/fleet-cloud/instances/${encodeURIComponent(vm.id)}`}
                      className="font-medium text-sky-400 hover:text-sky-300 inline-flex items-center gap-1.5"
                    >
                      {vm.name || vm.id.slice(0, 8)}
                    </Link>
                    <div className="text-xs text-slate-500 font-mono truncate max-w-[220px]">{vm.id}</div>
                  </td>
                  <td className="px-4 py-3">
                    <span className={`inline-block px-2 py-0.5 rounded border text-xs ${statusBadge(status)}`}>
                      {status}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-slate-300">
                    {vm.vcpus} vCPU / {vm.memory_mib} MiB
                  </td>
                  <td className="px-4 py-3 text-slate-400 font-mono text-xs">
                    {vm.guest_ip || '—'}
                  </td>
                  <td className="px-4 py-3">
                    <div className="flex justify-end gap-1">
                      <button
                        type="button"
                        title="Start"
                        onClick={() => void runAction(vm, startVm, 'Start')}
                        className={`p-2 rounded hover:bg-[color-mix(in_srgb,var(--machina-status-ok)_25%,transparent)] ${statusToneClass('ok')}`}
                      >
                        <Play className="w-4 h-4" />
                      </button>
                      <button
                        type="button"
                        title="Stop"
                        onClick={() => setPendingAction({ vm, fn: stopVm, label: 'Stop', message: `Stop instance '${vm.name}'? The guest OS will be powered off.` })}
                        className={`p-2 rounded hover:bg-[color-mix(in_srgb,var(--machina-status-error)_25%,transparent)] ${statusToneClass('error')}`}
                      >
                        <Square className="w-4 h-4" />
                      </button>
                      <button
                        type="button"
                        title="Reboot"
                        onClick={() => setPendingAction({ vm, fn: rebootVm, label: 'Reboot', message: `Reboot instance '${vm.name}'?` })}
                        className={`p-2 rounded hover:bg-[color-mix(in_srgb,var(--machina-status-warn)_25%,transparent)] ${statusToneClass('warn')}`}
                      >
                        <RotateCcw className="w-4 h-4" />
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

      <FleetCloudFooter />
      <ConfirmDialog
        open={pendingAction !== null}
        variant="warning"
        title={`${pendingAction?.label ?? ''} Instance`}
        message={pendingAction?.message ?? ''}
        confirmLabel={pendingAction?.label ?? 'Confirm'}
        onCancel={() => setPendingAction(null)}
        onConfirm={() => {
          const p = pendingAction
          setPendingAction(null)
          if (p) void runAction(p.vm, p.fn, p.label)
        }}
      />
    </PageLayout>
  )
}

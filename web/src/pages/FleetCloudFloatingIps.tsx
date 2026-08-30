// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router'
import {
  createVmPortForward,
  deleteVmPortForward,
  listVmPortForwards,
  listVms,
  type NativePortForward,
  type NativeVm,
} from '../api/nativeVms'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { statusActionLinkClasses } from '../utils/semanticColors'
import { Globe, Loader2, RefreshCw } from 'lucide-react'

const PROTOCOLS = ['tcp', 'udp'] as const

// Native "floating IPs" — there's no old external-cloud gate component in the
// way: the daemon's external-cloud-client integration has since been
// fully removed. There's no allocatable floating-IP pool; the native equivalent
// is a per-VM host_port -> vm_port NAT rule (controller::api::vms::port_forwards,
// already built) — see api/nativeVms.ts. Pick an instance, then manage its
// forwards.
export default function FleetCloudFloatingIpsPage() {
  return <FleetCloudFloatingIpsContent />
}

function FleetCloudFloatingIpsContent() {
  const toast = useToastContext()
  const [vms, setVms] = useState<NativeVm[]>([])
  const [vmId, setVmId] = useState('')
  const [forwards, setForwards] = useState<NativePortForward[]>([])
  const [loading, setLoading] = useState(true)
  const [protocol, setProtocol] = useState<string>(PROTOCOLS[0])
  const [hostPort, setHostPort] = useState('')
  const [vmPort, setVmPort] = useState('')
  const [description, setDescription] = useState('')
  const [creating, setCreating] = useState(false)

  const loadVms = useCallback(async () => {
    try {
      const list = await listVms()
      setVms(list)
      if (!vmId && list.length > 0) setVmId(list[0].id)
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [toast])

  useEffect(() => { void loadVms() }, [loadVms])

  const loadForwards = useCallback(async () => {
    if (!vmId) { setForwards([]); return }
    setLoading(true)
    try {
      const f = await listVmPortForwards(vmId)
      setForwards(f)
    } catch (e: unknown) {
      toast.error(formatUserError(e))
      setForwards([])
    } finally {
      setLoading(false)
    }
  }, [vmId, toast])

  useEffect(() => { void loadForwards() }, [loadForwards])

  const selectedVm = vms.find((v) => v.id === vmId)

  return (
    <PageLayout
      hideHeader
      className="w-full max-w-none"
      prepend={<><FleetCloudSubNav /></>}
    >
      <p className="apple-eyebrow">Fleet Cloud</p>
      <h1 className="page-title flex items-center gap-3">
        <Globe className="w-7 h-7 text-[var(--accent)]" />
        Floating IPs (port forwards)
      </h1>
      <p className="text-sm text-[var(--text-muted)]">
        No allocatable floating-IP pool natively — reach a VM's service from outside via a
        host_port → vm_port NAT rule instead.
      </p>

      <div className="rounded-2xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)] p-4 flex flex-wrap gap-3 items-end text-sm">
        <div>
          <label className="block text-xs text-[var(--text-muted)] mb-1">Instance</label>
          <select value={vmId} onChange={(e) => setVmId(e.target.value)}
            aria-label="Instance"
            className="input-field min-w-[12rem]">
            {vms.map((v) => <option key={v.id} value={v.id}>{v.name}</option>)}
          </select>
        </div>
        {selectedVm && (
          <span className="text-xs text-[var(--text-muted)] font-mono">guest IP: {selectedVm.guest_ip || 'unknown yet'}</span>
        )}
        <button type="button" onClick={() => void loadForwards()}
          className="ml-auto inline-flex items-center gap-1 px-3 py-1.5 rounded-lg border border-[var(--apple-hairline)]">
          <RefreshCw className="w-4 h-4" /> Refresh
        </button>
      </div>

      <div className="rounded-2xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)] p-4 space-y-3 text-sm">
        <h2 className="text-sm font-medium text-[var(--text-secondary)]">Add forward</h2>
        <div className="flex flex-wrap gap-3 items-end">
          <select value={protocol} onChange={(e) => setProtocol(e.target.value)}
            aria-label="Protocol"
            className="input-field">
            {PROTOCOLS.map((p) => <option key={p} value={p}>{p}</option>)}
          </select>
          <input aria-label="Host port" value={hostPort} onChange={(e) => setHostPort(e.target.value)} placeholder="Host port" type="number"
            className="w-28 input-field" />
          <input aria-label="VM port" value={vmPort} onChange={(e) => setVmPort(e.target.value)} placeholder="VM port" type="number"
            className="w-28 input-field" />
          <input aria-label="Description" value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Description (optional)"
            className="input-field min-w-[10rem]" />
          <button type="button" disabled={!vmId || !hostPort || !vmPort || creating}
            className="btn-primary text-sm disabled:opacity-40"
            onClick={async () => {
              const hp = Number.parseInt(hostPort, 10)
              const vp = Number.parseInt(vmPort, 10)
              if (!Number.isFinite(hp) || !Number.isFinite(vp)) {
                toast.warning('Ports must be numbers')
                return
              }
              setCreating(true)
              try {
                await createVmPortForward(vmId, { protocol, host_port: hp, vm_port: vp, description })
                toast.success('Forward added')
                setHostPort(''); setVmPort(''); setDescription('')
                void loadForwards()
              } catch (e: unknown) {
                toast.error(formatUserError(e))
              } finally {
                setCreating(false)
              }
            }}>{creating ? 'Adding…' : 'Add'}</button>
        </div>
      </div>

      {loading ? (
        <Loader2 className="w-8 h-8 animate-spin text-[var(--accent)] mx-auto" />
      ) : (
        <div className="rounded-2xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)] overflow-hidden">
          <table className="apple-table" aria-label="Port forwards">
            <thead>
              <tr>
                <th scope="col" className="px-3 py-2">Protocol</th>
                <th scope="col" className="px-3 py-2">Host port</th>
                <th scope="col" className="px-3 py-2">VM port</th>
                <th scope="col" className="px-3 py-2">Description</th>
                <th scope="col" className="px-3 py-2" />
              </tr>
            </thead>
            <tbody className="divide-y divide-[var(--apple-hairline)]">
              {forwards.map((f) => (
                <tr key={f.id}>
                  <td className="px-3 py-2 font-mono text-[var(--text-primary)]">{f.protocol}</td>
                  <td className="px-3 py-2">{f.host_port}</td>
                  <td className="px-3 py-2">{f.vm_port}</td>
                  <td className="px-3 py-2 text-[var(--text-muted)]">{f.description || '—'}</td>
                  <td className="px-3 py-2">
                    <button type="button" className={statusActionLinkClasses('error', 'text-xs')}
                      onClick={async () => {
                        if (!confirm(`Remove forward ${f.protocol}/${f.host_port} → ${f.vm_port}?`)) return
                        try {
                          await deleteVmPortForward(vmId, { protocol: f.protocol, host_port: f.host_port, vm_port: f.vm_port })
                          toast.success('Removed')
                          void loadForwards()
                        } catch (e: unknown) { toast.error(formatUserError(e)) }
                      }}>Remove</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {forwards.length === 0 && <p className="p-6 text-center text-[var(--text-muted)]">No port forwards for this instance.</p>}
        </div>
      )}
      {selectedVm && (
        <Link to={`/fleet-cloud/instances/${selectedVm.id}`} className="text-sm text-[var(--accent)] hover:underline">
          View instance
        </Link>
      )}
      <FleetCloudFooter />
    </PageLayout>
  )
}

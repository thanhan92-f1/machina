// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router'
import { createNetwork, deleteNetwork, listNetworks, type NativeNetwork } from '../api/nativeNetworks'
import { createPort, deletePort, listPorts, type NativePort } from '../api/nativePorts'
import { listVms, type NativeVm } from '../api/nativeVms'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import PageSkeleton from '../components/PageSkeleton'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { statusActionLinkClasses } from '../utils/semanticColors'
import { Network, Plus, RefreshCw } from 'lucide-react'

// Native networking overview — no <OpenStackGate> component to gate it behind
// any more (the daemon's external-OpenStack-client integration has since been
// fully removed). Networks (already pre-existing, libvirt-backed) and ports
// (this session's Neutron-port equivalent) only — no subnets/routers, since
// Machina has no native L3 routing layer. See FleetCloudTopology.tsx for the
// graph view of the same data.
export default function FleetCloudNetworkingPage() {
  return <FleetCloudNetworkingContent />
}

function FleetCloudNetworkingContent() {
  const toast = useToastContext()
  const [networks, setNetworks] = useState<NativeNetwork[]>([])
  const [ports, setPorts] = useState<NativePort[]>([])
  const [vms, setVms] = useState<NativeVm[]>([])
  const [loading, setLoading] = useState(true)
  const [newNetName, setNewNetName] = useState('')
  const [creatingNet, setCreatingNet] = useState(false)
  const [portNetId, setPortNetId] = useState('')
  const [portVmId, setPortVmId] = useState('')
  const [creatingPort, setCreatingPort] = useState(false)

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const [n, p, v] = await Promise.all([listNetworks(), listPorts(), listVms().catch(() => [])])
      setNetworks(n)
      setPorts(p)
      setVms(v)
      if (!portNetId && n.length > 0) setPortNetId(n[0].id)
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    } finally {
      setLoading(false)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [toast])

  useEffect(() => { void load() }, [load])

  const vmName = (id: string | null) => (id ? vms.find((v) => v.id === id)?.name || id.slice(0, 8) : '—')
  const netName = (id: string) => networks.find((n) => n.id === id)?.name || id.slice(0, 8)

  if (loading) return <PageSkeleton />

  return (
    <PageLayout
      hideHeader
      className="max-w-4xl"
      prepend={<><FleetCloudSubNav /></>}
    >
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-semibold flex items-center gap-2">
          <Network className="w-7 h-7 text-sky-400" />
          Networking
        </h1>
        <button type="button" onClick={() => void load()}
          className="inline-flex items-center gap-1 px-3 py-1.5 rounded-lg border border-slate-600 text-sm">
          <RefreshCw className="w-4 h-4" /> Refresh
        </button>
      </div>

      <section className="rounded-xl border border-slate-700 p-4 space-y-3">
        <h2 className="text-sm font-medium text-slate-300 flex items-center gap-2"><Plus className="w-4 h-4" /> Create network</h2>
        <div className="flex flex-wrap gap-2 items-end">
          <input aria-label="Network name" value={newNetName} onChange={(e) => setNewNetName(e.target.value)} placeholder="Name"
            className="px-2 py-1.5 rounded-lg bg-slate-900 border border-slate-700 text-sm" />
          <button type="button" disabled={!newNetName.trim() || creatingNet}
            className="px-3 py-1.5 rounded-lg bg-sky-600 text-white text-sm disabled:opacity-40"
            onClick={async () => {
              setCreatingNet(true)
              try {
                await createNetwork({ name: newNetName.trim() })
                toast.success('Network created')
                setNewNetName('')
                void load()
              } catch (e: unknown) { toast.error(formatUserError(e)) } finally { setCreatingNet(false) }
            }}>{creatingNet ? 'Creating…' : 'Create'}</button>
        </div>
        <div className="overflow-x-auto rounded-lg border border-slate-800">
          <table className="w-full text-sm" aria-label="Networks">
            <thead className="bg-slate-900 text-slate-400 text-left">
              <tr>
                <th scope="col" className="px-3 py-2">Name</th>
                <th scope="col" className="px-3 py-2">Backend</th>
                <th scope="col" className="px-3 py-2">VLAN</th>
                <th scope="col" className="px-3 py-2" />
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800">
              {networks.map((n) => (
                <tr key={n.id}>
                  <td className="px-3 py-2 text-slate-200">
                    <Link to={`/fleet-cloud/networks/${n.id}`} className="hover:text-sky-300 hover:underline">{n.name}</Link>
                  </td>
                  <td className="px-3 py-2 text-slate-400">{n.backend}</td>
                  <td className="px-3 py-2 text-slate-400">{n.vlan_id ?? '—'}</td>
                  <td className="px-3 py-2">
                    <button type="button" className={statusActionLinkClasses('error', 'text-xs')}
                      onClick={async () => {
                        if (!confirm(`Delete network ${n.name}?`)) return
                        try {
                          await deleteNetwork(n.id)
                          toast.success('Deleted')
                          void load()
                        } catch (e: unknown) { toast.error(formatUserError(e)) }
                      }}>Delete</button>
                  </td>
                </tr>
              ))}
              {networks.length === 0 && (
                <tr><td colSpan={4} className="px-3 py-4 text-center text-slate-500">No networks.</td></tr>
              )}
            </tbody>
          </table>
        </div>
      </section>

      <section className="rounded-xl border border-slate-700 p-4 space-y-3">
        <h2 className="text-sm font-medium text-slate-300 flex items-center gap-2"><Plus className="w-4 h-4" /> Create port</h2>
        <div className="flex flex-wrap gap-2 items-end">
          <select value={portNetId} onChange={(e) => setPortNetId(e.target.value)}
            aria-label="Network" className="px-2 py-1.5 rounded-lg bg-slate-900 border border-slate-700 text-sm min-w-[10rem]">
            {networks.map((n) => <option key={n.id} value={n.id}>{n.name}</option>)}
          </select>
          <select value={portVmId} onChange={(e) => setPortVmId(e.target.value)}
            aria-label="VM (optional)" className="px-2 py-1.5 rounded-lg bg-slate-900 border border-slate-700 text-sm min-w-[10rem]">
            <option value="">Unbound port</option>
            {vms.map((v) => <option key={v.id} value={v.id}>{v.name}</option>)}
          </select>
          <button type="button" disabled={!portNetId || creatingPort}
            className="px-3 py-1.5 rounded-lg bg-sky-600 text-white text-sm disabled:opacity-40"
            onClick={async () => {
              setCreatingPort(true)
              try {
                await createPort({ network_id: portNetId, vm_id: portVmId || undefined })
                toast.success('Port created')
                void load()
              } catch (e: unknown) { toast.error(formatUserError(e)) } finally { setCreatingPort(false) }
            }}>{creatingPort ? 'Creating…' : 'Create'}</button>
        </div>
        <div className="overflow-x-auto rounded-lg border border-slate-800">
          <table className="w-full text-sm" aria-label="Ports">
            <thead className="bg-slate-900 text-slate-400 text-left">
              <tr>
                <th scope="col" className="px-3 py-2">Network</th>
                <th scope="col" className="px-3 py-2">VM</th>
                <th scope="col" className="px-3 py-2">MAC</th>
                <th scope="col" className="px-3 py-2">Status</th>
                <th scope="col" className="px-3 py-2" />
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-800 font-mono text-xs">
              {ports.map((p) => (
                <tr key={p.id}>
                  <td className="px-3 py-2">{netName(p.network_id)}</td>
                  <td className="px-3 py-2">
                    {p.vm_id ? <Link to={`/fleet-cloud/instances/${p.vm_id}`} className="text-sky-400 hover:underline">{vmName(p.vm_id)}</Link> : '—'}
                  </td>
                  <td className="px-3 py-2 text-slate-400">{p.mac_address || '—'}</td>
                  <td className="px-3 py-2 text-slate-400">{p.status}</td>
                  <td className="px-3 py-2">
                    <button type="button" className={statusActionLinkClasses('error', 'text-xs')}
                      onClick={async () => {
                        if (!confirm('Delete this port?')) return
                        try {
                          await deletePort(p.id)
                          toast.success('Deleted')
                          void load()
                        } catch (e: unknown) { toast.error(formatUserError(e)) }
                      }}>Delete</button>
                  </td>
                </tr>
              ))}
              {ports.length === 0 && (
                <tr><td colSpan={5} className="px-3 py-4 text-center text-slate-500">No ports.</td></tr>
              )}
            </tbody>
          </table>
        </div>
      </section>

      <FleetCloudFooter />
    </PageLayout>
  )
}

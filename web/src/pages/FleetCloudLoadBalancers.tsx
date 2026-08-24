// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router'
import { Loader2, Plus, Scale, Trash2 } from 'lucide-react'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import EmptyState from '../components/EmptyState'
import { listPlatformHosts, type PlatformHost } from '../api/platform'
import {
  createLoadBalancer,
  deleteLoadBalancer,
  listLoadBalancers,
  type NativeLoadBalancer,
} from '../api/nativeLoadBalancers'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { statusActionLinkClasses, statusToneClass } from '../utils/semanticColors'

// Native L4 load balancer — like the other rewired /fleet-cloud/* pages, this
// no longer depends on a wired external cloud (see
// api/nativeLoadBalancers.ts). There's no old external-cloud gate component to wrap it
// in any more either: the daemon's external-cloud-client integration has
// since been fully removed, so this page always renders.
export default function FleetCloudLoadBalancersPage() {
  return <FleetCloudLoadBalancersContent />
}

function FleetCloudLoadBalancersContent() {
  const toast = useToastContext()
  const [lbs, setLbs] = useState<NativeLoadBalancer[]>([])
  const [hosts, setHosts] = useState<PlatformHost[]>([])
  const [loading, setLoading] = useState(true)
  const [name, setName] = useState('')
  const [hostId, setHostId] = useState('')
  const [listenerPort, setListenerPort] = useState('8080')

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const [lbR, hostR] = await Promise.all([
        listLoadBalancers(),
        listPlatformHosts().catch(() => []),
      ])
      setLbs(lbR)
      setHosts(hostR)
    } catch (e: unknown) {
      toast.error(formatUserError(e))
      setLbs([])
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => { void load() }, [load])

  return (
    <PageLayout hideHeader prepend={<><FleetCloudSubNav /></>}>
      <h1 className="text-2xl font-semibold flex items-center gap-2">
        <Scale className={`w-7 h-7 ${statusToneClass('ok')}`} /> Load balancers
      </h1>
      <p className="text-slate-400 text-sm">
        Native, kernel-level L4 (TCP/UDP) load balancing — a weighted round-robin iptables rule set on the
        chosen host, no external cloud or amphora VM required.
      </p>

      <div className="rounded-xl border border-slate-700 p-4 flex flex-wrap gap-3 items-end">
        <div>
          <label className="block text-xs text-slate-500 mb-1">Name</label>
          <input value={name} onChange={(e) => setName(e.target.value)} aria-label="Name" className="px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-sm" />
        </div>
        <div>
          <label className="block text-xs text-slate-500 mb-1">Host</label>
          <select value={hostId} onChange={(e) => setHostId(e.target.value)}
            aria-label="Host"
            className="px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-sm min-w-[14rem]">
            <option value="">Select host…</option>
            {hosts.map((h) => (
              <option key={h.id} value={h.id}>{h.hostname}</option>
            ))}
          </select>
        </div>
        <div>
          <label className="block text-xs text-slate-500 mb-1">Listener port</label>
          <input value={listenerPort} onChange={(e) => setListenerPort(e.target.value)} aria-label="Listener port"
            className="w-24 px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-sm" />
        </div>
        <button type="button" disabled={!name.trim() || !hostId || !Number(listenerPort)}
          className="px-3 py-1.5 rounded-lg bg-emerald-600 text-white text-sm disabled:opacity-40 inline-flex items-center gap-1"
          onClick={async () => {
            try {
              await createLoadBalancer({ name: name.trim(), host_id: hostId, listener_port: Number(listenerPort) })
              toast.success('Load balancer created')
              setName('')
              void load()
            } catch (e: unknown) { toast.error(formatUserError(e)) }
          }}>
          <Plus className="w-4 h-4" /> Create
        </button>
      </div>

      {loading ? (
        <Loader2 className="w-8 h-8 animate-spin text-sky-400 mx-auto" />
      ) : lbs.length === 0 ? (
        <EmptyState title="No load balancers" description="Create one above — pick a host and listener port, then add members." />
      ) : (
        <div className="overflow-x-auto rounded-xl border border-slate-700">
          <table className="w-full text-sm" aria-label="Load balancers">
            <thead className="bg-slate-900/80 text-slate-400 text-left">
              <tr>
                <th scope="col" className="px-3 py-2">Name</th>
                <th scope="col" className="px-3 py-2">Listener</th>
                <th scope="col" className="px-3 py-2">Host</th>
                <th scope="col" className="px-3 py-2">Status</th>
                <th scope="col" className="px-3 py-2" />
              </tr>
            </thead>
            <tbody>
              {lbs.map((lb) => (
                <tr key={lb.id} className="border-t border-slate-800">
                  <td className="px-3 py-2">
                    <Link to={`/fleet-cloud/load-balancers/${lb.id}`} className="text-sky-400 hover:underline">{lb.name}</Link>
                  </td>
                  <td className="px-3 py-2 font-mono">{lb.protocol}/{lb.listener_port}</td>
                  <td className="px-3 py-2 font-mono text-xs">{hosts.find((h) => h.id === lb.host_id)?.hostname ?? lb.host_id}</td>
                  <td className="px-3 py-2">{lb.status}</td>
                  <td className="px-3 py-2 text-right">
                    <button type="button" aria-label="Delete" className={statusActionLinkClasses('error', 'inline-flex items-center gap-1')}
                      onClick={async () => {
                        if (!confirm(`Delete ${lb.name}?`)) return
                        try {
                          await deleteLoadBalancer(lb.id)
                          toast.success('Deleted')
                          void load()
                        } catch (e: unknown) { toast.error(formatUserError(e)) }
                      }}>
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <FleetCloudFooter />
    </PageLayout>
  )
}

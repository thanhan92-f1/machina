// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useEffect, useState } from 'react'
import { Link } from 'react-router'
import { Cloud, ExternalLink } from 'lucide-react'
import { usePlatformInfo } from '../../contexts/PlatformInfoContext'
import type { PlatformVm } from '../../api/platform'
import { findOpenStackInstanceForVm, findOpenStackNetworkByName } from '../../utils/platformOpenStackLinks'
import { useOpenStackConnection } from '../../hooks/useOpenStackConnection'
import { statusActionLinkClasses } from '../../utils/semanticColors'

export function PlatformOpenStackVmLink({ vm }: { vm: Pick<PlatformVm, 'id' | 'name'> }) {
  const { info } = usePlatformInfo()
  const { phase } = useOpenStackConnection()
  const enabled = Boolean(info?.openstack?.enabled)
  const [instanceId, setInstanceId] = useState<string | null>(null)
  const [status, setStatus] = useState<string | null>(null)

  useEffect(() => {
    if (!enabled) return
    let cancelled = false
    void findOpenStackInstanceForVm(vm.name, vm.id).then((inst) => {
      if (cancelled) return
      setInstanceId(inst?.id ?? null)
      setStatus(inst?.status ?? null)
    })
    return () => { cancelled = true }
  }, [enabled, vm.id, vm.name])

  if (!enabled) return null

  if (phase !== 'live') {
    return (
      <Link to="/openstack" className={`text-xs inline-flex items-center gap-1 ${statusActionLinkClasses('warn', 'hover:opacity-90')}`}>
        <Cloud className="w-3 h-3" />
        OpenStack {phase} — open operator UI →
      </Link>
    )
  }

  return (
    <div className="flex flex-wrap items-center gap-2 text-sm">
      {instanceId ? (
        <Link
          to={`/openstack/instances/${instanceId}`}
          className="inline-flex items-center gap-1.5 text-sky-400 hover:text-sky-300"
        >
          <Cloud className="w-3.5 h-3.5" />
          OpenStack instance{status ? ` (${status})` : ''}
          <ExternalLink className="w-3 h-3 opacity-60" />
        </Link>
      ) : (
        <Link to="/openstack/instances" className="inline-flex items-center gap-1.5 text-slate-500 hover:text-sky-400">
          <Cloud className="w-3.5 h-3.5" />
          Search in OpenStack
        </Link>
      )}
      <Link to="/openstack/migrations" className="text-slate-500 hover:text-sky-400 text-xs">
        Migrations →
      </Link>
    </div>
  )
}

export function PlatformOpenStackNetworkLink({ networkName }: { networkName: string }) {
  const { info } = usePlatformInfo()
  const enabled = Boolean(info?.openstack?.enabled)
  const [networkId, setNetworkId] = useState<string | null>(null)

  useEffect(() => {
    if (!enabled || !networkName) return
    let cancelled = false
    void findOpenStackNetworkByName(networkName).then((n) => {
      if (!cancelled) setNetworkId(n?.id ?? null)
    })
    return () => { cancelled = true }
  }, [enabled, networkName])

  if (!enabled || !networkId) return null

  return (
    <Link
      to={`/openstack/networks/${networkId}`}
      className="inline-flex items-center gap-1 text-xs text-sky-400 hover:text-sky-300 mt-1"
    >
      <Cloud className="w-3 h-3" />
      Neutron network
      <ExternalLink className="w-2.5 h-2.5 opacity-60" />
    </Link>
  )
}

export function PlatformClassicToolLinks({
  tools,
}: {
  tools: Array<{ title: string; href: string; description?: string }>
}) {
  if (tools.length === 0) return null
  return (
    <ul className="grid gap-2 sm:grid-cols-2">
      {tools.map((t) => (
        <li key={t.href}>
          <Link
            to={t.href}
            className="block rounded-xl border border-white/[0.06] bg-slate-950/30 px-3 py-2.5 hover:border-sky-500/30 transition"
          >
            <span className="text-sm font-medium text-slate-200 flex items-center gap-1.5">
              {t.title}
              <ExternalLink className="w-3 h-3 text-slate-500" />
            </span>
            {t.description && <span className="text-xs text-slate-500 mt-0.5 block leading-relaxed">{t.description}</span>}
          </Link>
        </li>
      ))}
    </ul>
  )
}

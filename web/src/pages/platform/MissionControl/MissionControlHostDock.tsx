// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { Link } from 'react-router'
import { Monitor, X } from 'lucide-react'
import type { PlatformHost, PlatformVm } from '../../../api/platform'
import { hostStateTone } from '../../../utils/semanticColors'
import { cinemaHubPath } from '../../../utils/consoleExperienceMode'
import { StatusLed, SegmentedMeter } from './MissionControlRackKit'

type Props = {
  host: PlatformHost
  vms: PlatformVm[]
  onClose: () => void
}

function Spec({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-3 px-2.5 py-2 bg-[var(--glass-bg)]">
      <span className="text-[10px] font-mono uppercase tracking-wider text-[var(--text-muted)]">{label}</span>
      <b className="font-mono text-[11.5px] font-medium text-right truncate">{value}</b>
    </div>
  )
}

/** Host inspector — right-column dock shown when a host (not a VM) is selected in the rack strip. */
export default function MissionControlHostDock({ host, vms, onClose }: Props) {
  const tone = hostStateTone(host.state, host.fenced, host.maintenance_mode)
  const memPct = host.memory_total_mib > 0 ? (host.memory_used_mib / host.memory_total_mib) * 100 : 0

  return (
    <aside className="w-full lg:w-[320px] shrink-0 space-y-4" data-testid="mission-control-host-dock">
      <div className="flex items-center gap-2">
        <StatusLed tone={tone} />
        <span className="text-[10px] font-mono uppercase tracking-wider text-[var(--text-muted)]">
          {tone === 'ok' ? 'Healthy' : tone === 'warn' ? 'Pressure' : tone === 'error' ? 'Unreachable' : host.state}
        </span>
        <span className="flex-1" />
        <button type="button" className="text-[10px] font-mono uppercase tracking-wider text-[var(--text-muted)] hover:text-[var(--text-primary)]" onClick={onClose}>
          <X className="w-3.5 h-3.5 inline mr-1" />Close
        </button>
      </div>
      <p className="font-mono text-[17px] font-medium tracking-tight truncate">{host.hostname}</p>

      <div className="rounded-lg border border-white/[0.08] overflow-hidden divide-y divide-white/[0.06]">
        <Spec label="Address" value={host.address} />
        <Spec label="Site" value={host.site || '—'} />
        <Spec label="Rack" value={host.rack ? `${host.rack}${host.rack_u ? ` · U${host.rack_u}` : ''}` : '—'} />
        <Spec label="Machines" value={String(host.vm_count)} />
      </div>

      <div className="flex gap-5">
        <div className="flex-1">
          <div className="flex justify-between text-[10px] font-mono uppercase text-[var(--text-muted)] mb-1">
            <span>CPU</span><b className="text-[var(--text-primary)] normal-case">{Math.round(host.cpu_percent ?? 0)}%</b>
          </div>
          <SegmentedMeter percent={host.cpu_percent ?? 0} />
        </div>
        <div className="flex-1">
          <div className="flex justify-between text-[10px] font-mono uppercase text-[var(--text-muted)] mb-1">
            <span>MEM</span><b className="text-[var(--text-primary)] normal-case">{Math.round(memPct)}%</b>
          </div>
          <SegmentedMeter percent={memPct} />
        </div>
      </div>

      <div>
        <p className="text-[10px] font-mono uppercase tracking-wider text-[var(--text-muted)] mb-2">Machines · {vms.length}</p>
        <div className="space-y-1.5">
          {vms.map((vm) => (
            <div key={vm.id} className="flex items-center gap-2 px-2.5 py-1.5 rounded-md border border-white/[0.08] bg-[var(--glass-bg)]">
              <StatusLed tone={vm.observed_state === 'running' ? 'ok' : 'neutral'} />
              <span className="flex-1 min-w-0 font-mono text-[11.5px] font-medium truncate">{vm.name}</span>
              <Link
                to={cinemaHubPath(vm.id)}
                className="inline-flex items-center gap-1 text-[10px] font-mono uppercase text-[var(--text-muted)] hover:text-[var(--text-primary)]"
              >
                <Monitor className="w-3 h-3" />Console
              </Link>
            </div>
          ))}
          {vms.length === 0 && <p className="text-xs text-[var(--text-muted)]">No machines placed on this host.</p>}
        </div>
      </div>

      <div className="flex flex-wrap gap-2">
        <Link to={`/platform/hosts/${host.id}`} className="btn-primary text-xs">Open host</Link>
        <Link to={`/platform/vms?lens=topology&host=${encodeURIComponent(host.id)}`} className="btn-secondary text-xs">Machine Finder</Link>
      </div>
    </aside>
  )
}

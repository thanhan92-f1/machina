// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { Link } from 'react-router'
import { Server, ChevronRight } from 'lucide-react'
import LivingMachineCard from '../MachineFinder/LivingMachineCard'
import { openCenterPopout } from '../../../utils/platformCenterPopout'
import { cinemaPopoutPath } from '../../../utils/consoleExperienceMode'
import { hostStateTone } from '../../../utils/semanticColors'
import type { MissionControlFleetState } from './useMissionControlFleet'
import { StatusLed, SegmentedMeter, BayRow } from './MissionControlRackKit'

type Props = {
  state: MissionControlFleetState
  selectedHostId?: string | null
  onSelectHost?: (hostId: string | null) => void
}

export default function HostMachinePanels({ state, selectedHostId, onSelectHost }: Props) {
  const { hosts, vmsByHost, selectedVmId, setSelectedVmId, setDragVmId, setSshVm } = state

  if (state.loading) {
    return (
      <section className="rounded-xl border border-white/[0.08] bg-slate-900/40 p-6 text-center">
        <p className="text-slate-400">Loading host inventory…</p>
      </section>
    )
  }

  if (hosts.length === 0) {
    return (
      <section
        className="rounded-xl border border-dashed border-white/[0.14] p-9 text-center"
        style={{ background: 'repeating-linear-gradient(-45deg, transparent, transparent 9px, rgba(255,255,255,0.012) 9px, rgba(255,255,255,0.012) 18px)' }}
      >
        <Server className="w-6 h-6 mx-auto text-[var(--text-muted)]" />
        <h3 className="text-[15px] font-semibold mt-3 mb-1">No hosts attached</h3>
        <p className="text-sm text-[var(--text-muted)] max-w-[44ch] mx-auto mb-4">
          Once a host runs machina-daemon with valid controller credentials, it claims a slot here within seconds.
        </p>
        <Link to="/platform/enroll" className="btn-primary text-sm">Add host</Link>
      </section>
    )
  }

  return (
    <section data-testid="host-machine-panels" className="space-y-4">
      <div className="flex items-center justify-between gap-2">
        <h2 className="text-sm font-semibold uppercase tracking-wider text-slate-400">Hosts / Machines</h2>
        <span className="text-xs text-slate-500">Grouped</span>
      </div>
      {hosts.map((host) => {
        const hostVms = vmsByHost.get(host.id) ?? []
        const tone = hostStateTone(host.state, host.fenced, host.maintenance_mode)
        const memPct = host.memory_total_mib > 0 ? (host.memory_used_mib / host.memory_total_mib) * 100 : 0
        const selected = selectedHostId === host.id
        return (
          <article key={host.id} className="mc-host-panel rounded-2xl border border-white/[0.08] bg-slate-950/40 p-4 space-y-3">
            <button
              type="button"
              className={`w-full flex flex-wrap items-center gap-4 -m-1 p-1 rounded-lg text-left transition-colors ${selected ? 'bg-[var(--machina-accent)]/[0.08]' : 'hover:bg-white/[0.03]'}`}
              onClick={() => onSelectHost?.(selected ? null : host.id)}
              data-testid="mc-host-strip"
            >
              <span className="flex items-center gap-2 min-w-0 w-44 shrink-0">
                <StatusLed tone={tone} />
                <span className="min-w-0">
                  <span className="block font-mono text-[13px] font-medium truncate">{host.hostname}</span>
                  <span className="block font-mono text-[10px] text-[var(--text-muted)] truncate">{host.site || '—'} · {host.address}</span>
                </span>
              </span>

              <BayRow count={hostVms.length} tone={tone} />

              <span className="flex-1 min-w-[160px] flex gap-5">
                <span className="flex-1 min-w-[64px] max-w-[150px]">
                  <span className="flex justify-between text-[10px] font-mono uppercase text-[var(--text-muted)] mb-1">
                    <span>CPU</span><b className="text-[var(--text-primary)] normal-case">{Math.round(host.cpu_percent ?? 0)}%</b>
                  </span>
                  <SegmentedMeter percent={host.cpu_percent ?? 0} segments={10} />
                </span>
                <span className="flex-1 min-w-[64px] max-w-[150px]">
                  <span className="flex justify-between text-[10px] font-mono uppercase text-[var(--text-muted)] mb-1">
                    <span>MEM</span><b className="text-[var(--text-primary)] normal-case">{Math.round(memPct)}%</b>
                  </span>
                  <SegmentedMeter percent={memPct} segments={10} />
                </span>
              </span>

              <span className="text-xs text-[var(--text-muted)] shrink-0">{host.state}</span>
              <ChevronRight className={`w-4 h-4 shrink-0 text-[var(--text-muted)] transition-transform ${selected ? 'rotate-90' : ''}`} />
            </button>
            {hostVms.length > 0 ? (
              <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-3">
                {hostVms.map((vm) => (
                  <LivingMachineCard
                    key={vm.id}
                    vm={vm}
                    selected={selectedVmId === vm.id}
                    overlay="default"
                    onSelect={() => setSelectedVmId(vm.id)}
                    onDragStart={() => setDragVmId(vm.id)}
                    onDragEnd={() => setDragVmId(null)}
                    onSsh={() => setSshVm(vm)}
                    onDoubleClickTheatre={() => openCenterPopout(cinemaPopoutPath(vm.id))}
                  />
                ))}
              </div>
            ) : (
              <p className="text-sm text-slate-500">No machines on this host.</p>
            )}
            <footer className="flex flex-wrap gap-2 pt-1">
              <Link to={`/platform/hosts/${host.id}`} className="btn-secondary text-xs">Open host</Link>
              <Link to={`/platform/vms?lens=topology&host=${encodeURIComponent(host.id)}`} className="btn-secondary text-xs">Machine Finder</Link>
            </footer>
          </article>
        )
      })}
      {vmsByHost.has('__unassigned__') && (
        <article className="mc-host-panel rounded-2xl border border-dashed border-white/[0.12] p-4">
          <h3 className="text-sm font-medium text-slate-300 mb-3">Unassigned machines</h3>
          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 gap-3">
            {(vmsByHost.get('__unassigned__') ?? []).map((vm) => (
              <LivingMachineCard
                key={vm.id}
                vm={vm}
                selected={selectedVmId === vm.id}
                overlay="default"
                onSelect={() => setSelectedVmId(vm.id)}
                onDragStart={() => setDragVmId(vm.id)}
                onDragEnd={() => setDragVmId(null)}
                onSsh={() => setSshVm(vm)}
                onDoubleClickTheatre={() => openCenterPopout(cinemaPopoutPath(vm.id))}
              />
            ))}
          </div>
        </article>
      )}
    </section>
  )
}

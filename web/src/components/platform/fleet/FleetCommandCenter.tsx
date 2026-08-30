// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useEffect, useState } from 'react'
import { Link } from 'react-router'
import {
  Camera,
  Copy,
  Monitor,
  Pause,
  Play,
  Power,
  Square,
  Terminal,
  Trash2,
} from 'lucide-react'
import { runVmHealthCheck } from '../../../api/platform'
import VmStatusBadge from '../../VmStatusBadge'
import { formatVmMemoryGiB } from '../../../utils/vmVisual'
import { useToastContext } from '../../../contexts/ToastContext'
import ConsoleTheatrePreview from './ConsoleTheatrePreview'
import VmConsoleQuickLinks from './VmConsoleQuickLinks'
import { cinemaHubPath, studioHubPath } from '../../../utils/consoleExperienceMode'
import { DetailPanel, MetricList, type MetricItem } from '../DetailPanel'
import type { FleetCommandCenterProps } from './fleetCommandCenterTypes'

export default function FleetCommandCenter({
  selectedVm,
  hosts,
  hostMap,
  onSsh,
  onMigrate,
  onPower,
  onSnapshot,
  onDelete,
  onAdopt,
  showTheatrePreview = false,
  className = '',
  testId = 'fleet-command-center',
}: FleetCommandCenterProps) {
  const toast = useToastContext()
  const [healthScore, setHealthScore] = useState<number | null>(null)
  const [healthLoading, setHealthLoading] = useState(false)

  useEffect(() => {
    if (!selectedVm) {
      setHealthScore(null)
      return
    }
    setHealthLoading(true)
    void runVmHealthCheck(selectedVm.id)
      // Number(x) || null would drop a real score of 0 (worst health). Keep 0.
      .then((h) => {
        const n = h.score != null ? Number(h.score) : NaN
        setHealthScore(Number.isFinite(n) ? n : null)
      })
      .catch(() => setHealthScore(null))
      .finally(() => setHealthLoading(false))
  }, [selectedVm?.id])

  if (!selectedVm) {
    return (
      <DetailPanel
        empty
        emptyMessage="Select a machine to open Command Center"
        className={`hidden xl:flex ${className}`}
        testId={testId}
      />
    )
  }

  const vmState = selectedVm.observed_state
  const running = vmState === 'running'
  const paused = vmState === 'paused'
  const shutoff = vmState === 'shutoff' || vmState === 'shut off'

  const metrics: MetricItem[] = [
    { label: 'Health', value: healthLoading ? '…' : (healthScore != null ? `${healthScore}/100` : '—') },
    { label: 'State', value: <VmStatusBadge state={vmState} /> },
    { label: 'vCPU', value: `${selectedVm.vcpus} cores` },
    { label: 'Memory', value: formatVmMemoryGiB(selectedVm.memory_mib) },
    { label: 'Source', value: <span className="capitalize">{selectedVm.inventory_source ?? 'libvirt'}</span>, span: true },
    ...(selectedVm.guest_ip ? [{ label: 'Guest IP', value: <span className="font-mono text-emerald-300/90">{selectedVm.guest_ip}</span>, span: true }] : []),
  ]

  const primaryPowerAction = running ? (
    <button
      type="button"
      className="btn-secondary text-xs py-1.5 px-3 w-full inline-flex items-center justify-center gap-1.5 border-amber-500/30 text-amber-200 hover:bg-amber-500/10"
      onClick={() => void onPower(selectedVm, 'shutdown')}
    >
      <Power className="w-3.5 h-3.5" /> Shutdown
    </button>
  ) : paused ? (
    <button
      type="button"
      className="btn-secondary text-xs py-1.5 px-3 w-full inline-flex items-center justify-center gap-1.5 border-[var(--apple-hairline)] text-emerald-200 hover:bg-emerald-500/10"
      onClick={() => void onPower(selectedVm, 'resume')}
    >
      <Play className="w-3.5 h-3.5" /> Resume
    </button>
  ) : (
    <button
      type="button"
      className="btn-secondary text-xs py-1.5 px-3 w-full inline-flex items-center justify-center gap-1.5 border-[var(--apple-hairline)] text-emerald-200 hover:bg-emerald-500/10"
      onClick={() => void onPower(selectedVm, 'start')}
    >
      <Play className="w-3.5 h-3.5" /> Start
    </button>
  )

  return (
    <DetailPanel
      title="Command Center"
      subtitle={selectedVm.name}
      statusBadge={<VmStatusBadge state={vmState} />}
      footer={
        <Link
          to={cinemaHubPath(selectedVm.id)}
          className="btn-primary text-sm w-full text-center inline-flex items-center justify-center gap-1.5"
        >
          <Monitor className="w-3.5 h-3.5" /> Open Cinema
        </Link>
      }
      className={className}
      testId={testId}
    >
      <MetricList items={metrics} />

      <div className="space-y-2">
        <p className="text-xs font-medium text-[var(--text-muted)]">Power</p>
        {primaryPowerAction}
        <div className="flex flex-wrap gap-1.5">
          {(running || paused) && (
            <ActionBtn icon={Square} label="Stop" onClick={() => void onPower(selectedVm, 'stop')} />
          )}
          {running && (
            <ActionBtn icon={Pause} label="Pause" onClick={() => void onPower(selectedVm, 'pause')} />
          )}
          {!running && !paused && !shutoff && (
            <ActionBtn icon={Power} label="Shutdown" onClick={() => void onPower(selectedVm, 'shutdown')} />
          )}
          <ActionBtn icon={Camera} label="Snapshot" onClick={() => void onSnapshot(selectedVm)} />
          {selectedVm.inventory_source !== 'kubevirt' && (
            <ActionBtn icon={Terminal} label="SSH" onClick={() => onSsh(selectedVm)} />
          )}
        </div>
        <div className="pt-1 border-t border-white/[0.06]">
          <button
            type="button"
            className="btn-secondary text-xs w-full inline-flex items-center justify-center gap-1 border-red-500/30 text-red-300 hover:bg-red-500/10"
            onClick={() => void onDelete(selectedVm)}
          >
            <Trash2 className="w-3 h-3" /> Delete VM
          </button>
        </div>
      </div>

      <div className="space-y-2">
        <p className="text-xs font-medium text-[var(--text-muted)]">Console</p>
        <VmConsoleQuickLinks vmId={selectedVm.id} running={running} />
        <div className="flex flex-wrap gap-2">
          <Link
            to={studioHubPath(selectedVm.id)}
            className="btn-secondary text-xs flex-1 text-center inline-flex items-center justify-center gap-1"
          >
            Studio
          </Link>
          <Link
            to={`/platform/vms/${selectedVm.id}`}
            className="btn-secondary text-xs flex-1 text-center inline-flex items-center justify-center gap-1"
          >
            Open VM detail
          </Link>
        </div>
      </div>

      {selectedVm.guest_ip && (
        <button
          type="button"
          className="btn-secondary text-xs w-full inline-flex items-center justify-center gap-1"
          onClick={() => {
            void navigator.clipboard.writeText(selectedVm.guest_ip!)
            toast.success('Guest IP copied')
          }}
        >
          <Copy className="w-3.5 h-3.5" /> Copy IP
        </button>
      )}

      {selectedVm.host_id && hosts.length > 1 && (
        <MigratePicker
          vm={selectedVm}
          hosts={hosts}
          hostMap={hostMap}
          onPick={(destId, destName) => onMigrate(selectedVm, destId, destName)}
        />
      )}

      {selectedVm.managed === false && (
        <button
          type="button"
          className="btn-secondary text-xs w-full"
          onClick={() => void onAdopt(selectedVm)}
        >
          Adopt discovered VM
        </button>
      )}

      {showTheatrePreview && running && (
        <ConsoleTheatrePreview vmId={selectedVm.id} vmName={selectedVm.name} />
      )}

      <div className="rounded-lg border border-emerald-900/40 bg-[var(--apple-surface)] p-3 text-xs text-emerald-100/90">
        <p className="font-medium text-emerald-200/90 mb-1">Zyra says</p>
        <p>
          {healthScore != null && healthScore < 70
            ? 'Health score is low — review backups and guest agent connectivity.'
            : selectedVm.guest_ip
              ? 'Machine is reachable — console and SSH are ready.'
              : 'No guest IP yet — check network and guest tools.'}
        </p>
      </div>
    </DetailPanel>
  )
}

function ActionBtn({
  icon: Icon,
  label,
  onClick,
}: {
  icon: typeof Play
  label: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      className="btn-secondary text-xs py-1 px-2 inline-flex items-center gap-1"
      onClick={onClick}
    >
      <Icon className="w-3 h-3" /> {label}
    </button>
  )
}

function MigratePicker({
  vm,
  hosts,
  hostMap,
  onPick,
}: {
  vm: FleetCommandCenterProps['selectedVm']
  hosts: FleetCommandCenterProps['hosts']
  hostMap: Map<string, string>
  onPick: (destId: string, destName: string) => void
}) {
  if (!vm) return null
  const candidates = hosts.filter((h) => h.id !== vm.host_id)
  if (candidates.length === 0) return null
  return (
    <div>
      <p className="text-xs font-medium text-[var(--text-muted)] mb-1">Migrate to</p>
      <select
        aria-label="Migrate VM to host"
        className="w-full text-xs rounded-lg bg-[var(--apple-surface)] border border-white/10 px-2 py-1.5"
        defaultValue=""
        onChange={(e) => {
          const id = e.target.value
          if (!id) return
          const host = hosts.find((h) => h.id === id)
          if (host) onPick(id, host.hostname)
          e.target.value = ''
        }}
      >
        <option value="">Select host…</option>
        {candidates.map((h) => (
          <option key={h.id} value={h.id}>
            {h.hostname} ({hostMap.get(h.id) ?? h.id.slice(0, 8)})
          </option>
        ))}
      </select>
    </div>
  )
}

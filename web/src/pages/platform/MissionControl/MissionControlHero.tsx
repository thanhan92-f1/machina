// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { AlertTriangle, Search } from 'lucide-react'
import { formatFleetDisplayTitle } from '../../../utils/fleetDisplayName'
import { dispatchOpenSpotlight } from '../../../utils/platformJarvisShell'
import type { MissionControlFleetState } from './useMissionControlFleet'
import { SegmentedMeter, type StatusTone } from './MissionControlRackKit'

type Props = {
  state: MissionControlFleetState
  warnings: number
}

const WORD_TONE: Record<'connecting' | 'offline' | 'degraded' | 'operational', StatusTone> = {
  connecting: 'neutral',
  offline: 'error',
  degraded: 'warn',
  operational: 'ok',
}

export default function MissionControlHero({ state, warnings }: Props) {
  const { cluster, hosts, running, onlineHosts, storagePct, needsAttention, attentionMode, setAttentionMode } = state
  const fleetTitle = formatFleetDisplayTitle(cluster, hosts)
  const healthy = !state.loading && hosts.length > 0 && warnings === 0 && onlineHosts === hosts.length

  const status: keyof typeof WORD_TONE = state.loading
    ? 'connecting'
    : state.error
      ? 'offline'
      : healthy
        ? 'operational'
        : 'degraded'
  const word = { connecting: 'Connecting…', offline: 'Offline', degraded: 'Degraded', operational: 'Operational' }[status]
  const tone = WORD_TONE[status]

  return (
    <header className="mc-hero flex flex-col lg:flex-row lg:items-end gap-5" data-testid="mission-control-hero">
      <div className="min-w-0">
        <p className="text-[10px] font-mono font-medium uppercase tracking-[0.16em] text-[var(--text-muted)] mb-1 truncate">{fleetTitle}</p>
        <h1
          className="text-[clamp(2.1rem,5vw,3.4rem)] leading-[0.95] font-bold uppercase tracking-tight"
          style={{ color: `var(--machina-status-${tone})` }}
        >
          {word}
        </h1>
        <div className="flex flex-wrap items-center gap-2 mt-2">
          {needsAttention > 0 && (
            <button
              type="button"
              className={`text-xs px-2.5 py-1 rounded-full border inline-flex items-center gap-1 ${attentionMode ? 'border-amber-400/50 bg-amber-500/20 text-amber-100' : 'border-white/10 text-[var(--text-muted)] hover:text-amber-200'}`}
              onClick={() => setAttentionMode(!attentionMode)}
              data-testid="attention-mode-toggle"
            >
              <AlertTriangle className="w-3 h-3" />
              {needsAttention} need attention
            </button>
          )}
          <button
            type="button"
            className="text-xs px-2.5 py-1 rounded-full border border-white/10 text-[var(--text-muted)] hover:text-[var(--text-primary)] inline-flex items-center gap-1.5"
            onClick={() => dispatchOpenSpotlight()}
          >
            <Search className="w-3 h-3" />
            Search fleet…
            <span className="font-mono text-[10px] opacity-60">⌘⎵</span>
          </button>
        </div>
      </div>

      <div className="flex-1 min-w-[280px] grid grid-cols-2 sm:grid-cols-4 gap-px rounded-xl border border-white/[0.08] overflow-hidden bg-white/[0.06]">
        {[
          { label: 'VMs running', value: state.loading ? '—' : String(running), pct: state.loading ? 0 : Math.min(100, running * 8), tone: 'info' as StatusTone },
          { label: 'Hosts online', value: state.loading ? '—' : `${onlineHosts}/${hosts.length}`, pct: hosts.length ? (onlineHosts / hosts.length) * 100 : 0 },
          { label: 'Memory used', value: storagePct != null ? `${Math.round(storagePct)}%` : '—', pct: storagePct ?? 0 },
          { label: 'Alerts', value: warnings ? String(warnings) : 'None', pct: Math.min(100, warnings * 25), tone: warnings ? 'warn' as StatusTone : 'ok' as StatusTone },
        ].map((g) => (
          <div key={g.label} className="bg-[var(--glass-bg)] px-3.5 py-2.5">
            <p className="text-[10px] font-mono uppercase tracking-wider text-[var(--text-muted)]">{g.label}</p>
            <div className="flex items-baseline gap-1 my-1.5">
              <b className="font-mono text-xl font-medium tracking-tight">{g.value}</b>
            </div>
            <SegmentedMeter percent={g.pct} tone={g.tone} />
          </div>
        ))}
      </div>
    </header>
  )
}

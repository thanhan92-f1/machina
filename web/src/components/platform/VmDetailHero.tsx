// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { Activity, Monitor, Shield, Sparkles, Terminal, Zap } from 'lucide-react'
import type { VmDetailBlocker } from '../../utils/vmDetailSpotlight'

type Props = {
  vmName: string
  observedState: string
  guestIp?: string
  hostLabel?: string
  healthScore?: number | null
  doctorScore?: number | null
  sshExposed?: boolean
  blockers?: VmDetailBlocker[]
  onOpenAccess?: () => void
  onOpenDoctor?: () => void
}

function stateTone(state: string): string {
  if (state === 'running') return 'text-[var(--verdant)]'
  if (state === 'paused') return 'text-[var(--amber)]'
  if (state === 'missing' || state === 'failed') return 'text-[var(--ember)]'
  return 'text-[var(--text-muted)]'
}

export default function VmDetailHero({
  vmName,
  observedState,
  guestIp,
  hostLabel,
  healthScore,
  doctorScore,
  sshExposed,
  blockers = [],
  onOpenAccess,
  onOpenDoctor,
}: Props) {
  const running = observedState === 'running'
  const blockerCount = blockers.length
  const ready = running && Boolean(guestIp?.trim()) && (sshExposed || !blockers.includes('ssh_not_exposed'))

  return (
    <header className="vm-detail-hero apple-page-header border-b border-[var(--apple-hairline)] pb-8 mb-2 animate-fade-in" data-testid="vm-detail-hero">
      <div className="min-w-0 flex-1">
        <p className="apple-eyebrow">Virtual machine</p>
        <h1 className="page-title truncate flex items-center gap-3">
          <Monitor className="w-8 h-8 text-[var(--text-muted)] shrink-0 hidden sm:block" strokeWidth={1.5} />
          {vmName}
        </h1>
        <p className="page-lede !mt-3 flex flex-wrap items-center gap-x-2 gap-y-1 !max-w-none">
          <span className={`inline-flex items-center gap-1.5 ${stateTone(observedState)}`}>
            {running && <span className="vm-detail-live-dot" aria-hidden />}
            {observedState}
          </span>
          {hostLabel && (
            <>
              <span className="text-[var(--text-faint)]">·</span>
              <span>{hostLabel}</span>
            </>
          )}
          {guestIp && (
            <>
              <span className="text-[var(--text-faint)]">·</span>
              <span className="font-mono text-[var(--verdant)]">{guestIp}</span>
            </>
          )}
        </p>
      </div>

      <div className="apple-page-actions">
        {ready ? (
          <span className="vm-detail-hero-pill vm-detail-hero-pill--ok">
            <Zap className="w-3.5 h-3.5" /> Ready to connect
          </span>
        ) : blockerCount > 0 ? (
          <button
            type="button"
            className="vm-detail-hero-pill vm-detail-hero-pill--warn"
            onClick={onOpenAccess}
          >
            <Sparkles className="w-3.5 h-3.5" />
            {blockerCount} blocker{blockerCount === 1 ? '' : 's'} — fix in Access
          </button>
        ) : null}
        {healthScore != null && !Number.isNaN(healthScore) && (
          <span className="vm-detail-hero-pill">
            <Activity className="w-3.5 h-3.5 text-[var(--link)]" />
            Health {healthScore}
          </span>
        )}
        {doctorScore != null && (
          <button type="button" className="vm-detail-hero-pill" onClick={onOpenDoctor}>
            <Shield className="w-3.5 h-3.5 text-[var(--accent)]" />
            Doctor {doctorScore}/100
          </button>
        )}
        {sshExposed && (
          <span className="vm-detail-hero-pill vm-detail-hero-pill--ok">
            <Terminal className="w-3.5 h-3.5" /> SSH exposed
          </span>
        )}
      </div>
    </header>
  )
}

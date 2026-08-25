// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import {
  Boxes,
  HardDrive,
  Monitor,
  Plus,
  Search,
  Shield,
  Sparkles,
  Terminal,
  Wrench,
} from 'lucide-react'
import { Link } from 'react-router'
import { cinemaHubPath } from '../../../utils/consoleExperienceMode'
import type { PlatformVm } from '../../../api/platform'

type Props = {
  onCreateVm: () => void
  lastVm?: PlatformVm | null
}

const CARDS: Array<{
  id: string
  label: string
  subtitle: string
  icon: typeof Plus
  href?: string
  action?: 'create' | 'console'
}> = [
  { id: 'create', label: 'Create VM', subtitle: '4-step wizard', icon: Plus, action: 'create' },
  { id: 'security', label: 'Security Center', subtitle: 'Firewall & risk', icon: Shield, href: '/platform/zeus/security' },
  { id: 'finder', label: 'Machine Finder', subtitle: 'Discover VMs', icon: Search, href: '/platform/vms' },
  { id: 'recovery', label: 'Recovery', subtitle: 'Snapshots', icon: HardDrive, href: '/platform/backups' },
  { id: 'gpu', label: 'GPU Command Center', subtitle: 'Scheduling', icon: Sparkles, href: '/platform/gpu' },
  { id: 'migrate', label: 'Migration Planner', subtitle: 'Drag & drop', icon: Boxes, href: '/platform/vms?lens=migration' },
  { id: 'golden', label: 'Golden Image Builder', subtitle: 'Templates', icon: Wrench, href: '/platform/templates' },
  { id: 'console', label: 'Machina Cinema', subtitle: 'Live console', icon: Monitor, action: 'console' },
  { id: 'live-wall', label: 'Live Preview Wall', subtitle: 'Fleet grid', icon: Monitor, href: '/platform/mission-control/live' },
  { id: 'trace', label: 'PacketWolf Trace', subtitle: 'Network path', icon: Terminal, href: '/platform/network-canvas' },
]

export default function MissionControlLaunchpad({ onCreateVm, lastVm }: Props) {
  return (
    <section data-testid="mission-control-launchpad">
      <div className="flex items-center gap-3 mb-2">
        <p className="text-[10px] font-mono font-medium uppercase tracking-[0.16em] text-[var(--text-muted)]">Operations</p>
        <span className="flex-1 h-px bg-white/[0.08]" />
      </div>
      <div
        className="grid gap-px rounded-xl border border-white/[0.08] overflow-hidden bg-white/[0.06]"
        style={{ gridTemplateColumns: 'repeat(auto-fill, minmax(210px, 1fr))' }}
      >
        {CARDS.map((card) => {
          const Icon = card.icon
          const inner = (
            <span className="flex items-center gap-2.5 px-3 py-2.5 bg-[var(--glass-bg)] hover:bg-white/[0.04] transition-colors w-full text-left">
              <span className="w-7 h-7 rounded-md flex items-center justify-center bg-black/20 border border-white/[0.08] text-[var(--text-muted)] shrink-0">
                <Icon className="w-3.5 h-3.5" strokeWidth={1.75} />
              </span>
              <span className="min-w-0">
                <span className="block text-[12.5px] font-medium truncate">{card.label}</span>
                <span className="block font-mono text-[10px] text-[var(--text-muted)] truncate">{card.subtitle}</span>
              </span>
            </span>
          )
          if (card.action === 'create') {
            return <button key={card.id} type="button" onClick={onCreateVm}>{inner}</button>
          }
          if (card.action === 'console') {
            const consoleHref = lastVm ? cinemaHubPath(lastVm.id) : '/platform/vms'
            return <Link key={card.id} to={consoleHref}>{inner}</Link>
          }
          return <Link key={card.id} to={card.href!}>{inner}</Link>
        })}
      </div>
    </section>
  )
}

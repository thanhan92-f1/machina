// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { Search } from 'lucide-react'
import { dispatchOpenSpotlight } from '../../utils/platformJarvisShell'

export default function LaunchpadHeroSearch() {
  return (
    <button
      type="button"
      className="w-full flex items-center gap-3 rounded-2xl border border-white/10 bg-white/[0.04] px-4 py-3.5 text-left hover:border-orange-500/30 hover:bg-white/[0.06] transition"
      onClick={() => dispatchOpenSpotlight()}
      data-testid="launchpad-hero-search"
    >
      <Search className="w-5 h-5 text-orange-400 shrink-0" />
      <span className="text-sm text-slate-400 flex-1">
        Open Grafana, Prometheus, PacketWolf, Argo CD…
      </span>
      <kbd className="hidden sm:inline text-[10px] text-slate-500 border border-white/10 rounded px-1.5 py-0.5">⌘K</kbd>
    </button>
  )
}

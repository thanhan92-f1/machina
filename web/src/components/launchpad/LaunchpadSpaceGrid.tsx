// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { Link } from 'react-router'
import { MacGlassPanel } from '../platform/mac/PlatformMacUi'
import { LAUNCHPAD_SPACES, spaceCounts, type LaunchpadSpaceId } from '../../utils/launchpadSpaces'
import type { LaunchpadApp } from '../../api/launchpad'

type Props = {
  apps: LaunchpadApp[]
}

const SPACE_GRADIENTS: Record<LaunchpadSpaceId, string> = {
  monitoring: 'from-orange-500 to-rose-600',
  virtualization: 'from-violet-500 to-indigo-600',
  security: 'from-emerald-500 to-teal-600',
  storage: 'from-sky-500 to-blue-600',
  developer: 'from-amber-500 to-orange-600',
  ai: 'from-fuchsia-500 to-purple-600',
  other: 'from-slate-500 to-slate-700',
}

export default function LaunchpadSpaceGrid({ apps }: Props) {
  const counts = spaceCounts(apps)

  return (
    <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3" data-testid="launchpad-spaces-grid">
      {LAUNCHPAD_SPACES.filter((space) => counts[space.id] > 0).map((space) => (
        <Link key={space.id} to={`/platform/launchpad/spaces/${space.id}`} className="block group">
          <MacGlassPanel className="h-full transition-all duration-200 hover:border-orange-500/30 hover:shadow-lg hover:shadow-orange-500/10 hover:scale-[1.01]">
            <div className="flex items-start gap-3">
              <div
                className={`w-11 h-11 rounded-2xl bg-gradient-to-br ${SPACE_GRADIENTS[space.id]} flex items-center justify-center text-white text-sm font-bold shadow-lg shadow-black/20`}
                aria-hidden
              >
                {space.label.charAt(0)}
              </div>
              <div className="min-w-0">
                <h3 className="text-sm font-semibold text-slate-50 group-hover:text-orange-200 transition-colors">
                  {space.label}
                </h3>
                <p className="text-xs text-slate-400 mt-0.5 line-clamp-2">{space.description}</p>
                <p className="text-xs text-orange-300/90 mt-2 font-medium">
                  {counts[space.id]} app{counts[space.id] === 1 ? '' : 's'}
                </p>
              </div>
            </div>
          </MacGlassPanel>
        </Link>
      ))}
    </div>
  )
}

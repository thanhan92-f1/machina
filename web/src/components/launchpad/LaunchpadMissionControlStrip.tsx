// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useEffect, useMemo, useState } from 'react'
import { Link } from 'react-router'
import { LayoutGrid, Loader2 } from 'lucide-react'
import { getLaunchpadConfig, listLaunchpadCatalog, type LaunchpadApp } from '../../api/launchpad'
import { gradientForName, LaunchpadAppIcon, MacGlassPanel } from '../platform/mac/PlatformMacUi'
import { LAUNCHPAD_SPACES, groupAppsBySpace } from '../../utils/launchpadSpaces'
import { openLaunchpadApp } from '../../utils/launchpadHelpers'

const APPS_PER_SPACE = 5

export default function LaunchpadMissionControlStrip() {
  const [apps, setApps] = useState<LaunchpadApp[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let cancelled = false
    // Config first (cached): avoids a guaranteed 503 on hosts without Hermes.
    void getLaunchpadConfig()
      .then((cfg) => (cfg.enabled ? listLaunchpadCatalog() : ([] as LaunchpadApp[])))
      .then((catalog) => {
        if (!cancelled) setApps(catalog.filter((a) => a.visibility?.published !== false))
      })
      .catch(() => {
        if (!cancelled) setApps([])
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [])

  const grouped = useMemo(() => groupAppsBySpace(apps), [apps])
  const spacesWithApps = LAUNCHPAD_SPACES.filter((s) => (grouped.get(s.id)?.length ?? 0) > 0)

  if (loading) {
    return (
      <MacGlassPanel>
        <div className="flex items-center gap-2 text-sm text-slate-400 py-2">
          <Loader2 className="w-4 h-4 animate-spin" />
          Loading infrastructure apps…
        </div>
      </MacGlassPanel>
    )
  }

  if (spacesWithApps.length === 0) return null

  return (
    <section className="space-y-4" data-testid="launchpad-mission-control-strip">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-sm font-semibold text-slate-100 inline-flex items-center gap-2">
            <LayoutGrid className="w-4 h-4 text-orange-400" />
            Infrastructure Apps
          </h2>
          <p className="text-xs text-slate-400 mt-1">Kubernetes services and consoles from Launchpad</p>
        </div>
        <Link to="/platform/launchpad" className="btn-secondary text-xs">
          Open Launchpad
        </Link>
      </div>
      <div className="grid gap-4 lg:grid-cols-2">
        {spacesWithApps.map((space) => {
          const spaceApps = (grouped.get(space.id) ?? []).slice(0, APPS_PER_SPACE)
          return (
            <MacGlassPanel key={space.id} className="space-y-3">
              <div className="flex items-center justify-between">
                <h3 className="text-xs font-semibold uppercase tracking-wide text-slate-300">{space.label}</h3>
                <Link to={`/platform/launchpad/spaces/${space.id}`} className="text-[11px] text-orange-300 hover:underline">
                  View all
                </Link>
              </div>
              <div className="flex flex-wrap gap-3">
                {spaceApps.map((app) => (
                  <LaunchpadAppIcon
                    key={app.id}
                    name={app.displayName}
                    gradient={gradientForName(app.displayName)}
                    icon={
                      <span className="text-lg font-bold">
                        {app.displayName.trim().charAt(0).toUpperCase()}
                      </span>
                    }
                    onClick={() => void openLaunchpadApp(app)}
                  />
                ))}
              </div>
            </MacGlassPanel>
          )
        })}
      </div>
    </section>
  )
}

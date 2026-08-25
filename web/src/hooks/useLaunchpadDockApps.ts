// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useEffect, useState } from 'react'
import { useLocation } from 'react-router'
import { getLaunchpadConfig, listLaunchpadFavorites, type LaunchpadApp } from '../api/launchpad'
import { LAUNCHPAD_FAVORITES_CHANGED } from '../utils/launchpadHelpers'

export function useLaunchpadDockApps(limit = 4): LaunchpadApp[] {
  const location = useLocation()
  const [apps, setApps] = useState<LaunchpadApp[]>([])

  useEffect(() => {
    if (!location.pathname.startsWith('/platform')) {
      setApps([])
      return
    }
    let cancelled = false
    const refresh = () => {
      // Check the (cached) launchpad config first: with Hermes not installed the
      // favorites endpoint 503s on every platform page otherwise.
      void getLaunchpadConfig()
        .then((cfg) => (cfg.enabled ? listLaunchpadFavorites() : ([] as LaunchpadApp[])))
        .then((favs) => {
          if (!cancelled) setApps(favs.slice(0, limit))
        })
        .catch(() => {
          if (!cancelled) setApps([])
        })
    }
    refresh()
    window.addEventListener(LAUNCHPAD_FAVORITES_CHANGED, refresh)
    return () => {
      cancelled = true
      window.removeEventListener(LAUNCHPAD_FAVORITES_CHANGED, refresh)
    }
  }, [location.pathname, limit])

  return apps
}

// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useEffect, useState } from 'react'
import { getCachedLaunchpadConfig, getLaunchpadConfig } from '../api/launchpad'

/**
 * Whether the Launchpad feature is enabled on the daemon (`GET /api/v1/launchpad/config`).
 * Defaults to `true` (from the cached config, if present) until the fetch resolves so the
 * nav item is not hidden on a slow first load; a backend `enabled: false` then hides it.
 */
export function useLaunchpadEnabled(): boolean {
  const [enabled, setEnabled] = useState<boolean>(() => getCachedLaunchpadConfig()?.enabled ?? true)

  useEffect(() => {
    let cancelled = false
    void getLaunchpadConfig()
      .then((cfg) => {
        if (!cancelled) setEnabled(cfg.enabled)
      })
      .catch(() => {
        /* keep default (enabled) on failure */
      })
    return () => { cancelled = true }
  }, [])

  return enabled
}

// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from 'react'
import {
  defaultSidebarVisibleForTier,
  loadPlatformDesktopTier,
  PLATFORM_DESKTOP_TIER_EVENT,
  type PlatformDesktopTier,
} from '../../../utils/platformDesktopTier'
import { JARVIS_SHELL_EVENT } from '../../../utils/platformJarvisShell'

type PlatformMacDesktopContextValue = {
  sidebarVisible: boolean
  sidebarCollapsed: boolean
  inspectorVisible: boolean
  cinemaChromeHidden: boolean
  toggleSidebar: () => void
  setSidebarVisible: (v: boolean) => void
  setSidebarCollapsed: (v: boolean) => void
  setCinemaChromeHidden: (v: boolean) => void
  toggleInspector: () => void
  setInspectorVisible: (v: boolean) => void
}

const PlatformMacDesktopContext = createContext<PlatformMacDesktopContextValue | null>(null)

const SIDEBAR_COLLAPSED_KEY = 'machina-platform-sidebar-collapsed'

function defaultSidebarCollapsedForTier(_tier: PlatformDesktopTier): boolean {
  try {
    const raw = localStorage.getItem(SIDEBAR_COLLAPSED_KEY)
    if (raw === '0') return false
    if (raw === '1') return true
  } catch {
    /* ignore */
  }
  return false
}

export function PlatformMacDesktopProvider({ children }: { children: ReactNode }) {
  const [sidebarVisible, setSidebarVisibleState] = useState(() => defaultSidebarVisibleForTier(loadPlatformDesktopTier()))
  const [sidebarCollapsed, setSidebarCollapsed] = useState(() => defaultSidebarCollapsedForTier(loadPlatformDesktopTier()))
  const [inspectorVisible, setInspectorVisible] = useState(true)
  const [cinemaChromeHidden, setCinemaChromeHidden] = useState(false)

  useEffect(() => {
    const applyTier = () => {
      const tier = loadPlatformDesktopTier()
      setSidebarVisible(defaultSidebarVisibleForTier(tier))
      try {
        /* collapsed preference is localStorage-owned; do not force-collapse on tier change */
      } catch {
        /* ignore */
      }
    }
    window.addEventListener(PLATFORM_DESKTOP_TIER_EVENT, applyTier)
    window.addEventListener(JARVIS_SHELL_EVENT, applyTier)
    return () => {
      window.removeEventListener(PLATFORM_DESKTOP_TIER_EVENT, applyTier)
      window.removeEventListener(JARVIS_SHELL_EVENT, applyTier)
    }
  }, [])

  const toggleSidebar = useCallback(() => setSidebarVisibleState((v) => !v), [])
  const setSidebarVisible = useCallback((v: boolean) => setSidebarVisibleState(v), [])
  const toggleInspector = useCallback(() => setInspectorVisible((v) => !v), [])

  const setCollapsed = useCallback((v: boolean) => {
    setSidebarCollapsed(v)
    localStorage.setItem(SIDEBAR_COLLAPSED_KEY, v ? '1' : '0')
  }, [])

  const value = useMemo(
    () => ({
      sidebarVisible,
      sidebarCollapsed,
      inspectorVisible,
      cinemaChromeHidden,
      toggleSidebar,
      setSidebarVisible,
      setSidebarCollapsed: setCollapsed,
      setCinemaChromeHidden,
      toggleInspector,
      setInspectorVisible,
    }),
    [sidebarVisible, sidebarCollapsed, inspectorVisible, cinemaChromeHidden, toggleSidebar, setSidebarVisible, setCollapsed, toggleInspector],
  )

  return <PlatformMacDesktopContext.Provider value={value}>{children}</PlatformMacDesktopContext.Provider>
}

export function usePlatformMacDesktop() {
  const ctx = useContext(PlatformMacDesktopContext)
  if (!ctx) throw new Error('usePlatformMacDesktop must be used within PlatformMacDesktopProvider')
  return ctx
}

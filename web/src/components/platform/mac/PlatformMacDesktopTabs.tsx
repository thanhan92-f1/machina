// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useEffect, useState, type MouseEvent } from 'react'
import { Link, useLocation, useNavigate } from 'react-router'
import { ExternalLink, Plus, X } from 'lucide-react'
import {
  loadPlatformDesktopTabs,
  platformDesktopTabActive,
  platformPageLabel,
  removePlatformDesktopTab,
  upsertPlatformDesktopTab,
  PLATFORM_DESKTOP_TABS_EVENT,
  type PlatformDesktopTab,
} from '../../../utils/platformDesktopTabs'
import { statusToneClass } from '../../../utils/semanticColors'
import { openCenterPopout, useCenterPopout } from '../../../utils/platformCenterPopout'
import { dispatchOpenSpotlight } from '../../../utils/platformJarvisShell'

export default function PlatformMacDesktopTabs() {
  const location = useLocation()
  const navigate = useNavigate()
  const { desktopTo } = useCenterPopout()
  const [tabs, setTabs] = useState<PlatformDesktopTab[]>(() => loadPlatformDesktopTabs())

  useEffect(() => {
    const refresh = () => setTabs(loadPlatformDesktopTabs())
    window.addEventListener(PLATFORM_DESKTOP_TABS_EVENT, refresh)
    return () => window.removeEventListener(PLATFORM_DESKTOP_TABS_EVENT, refresh)
  }, [])

  useEffect(() => {
    if (!location.pathname.startsWith('/platform')) return
    const label = platformPageLabel(location.pathname)
    setTabs(upsertPlatformDesktopTab({ path: location.pathname, label }))
  }, [location.pathname])

  const closeTab = (path: string, e: MouseEvent) => {
    e.preventDefault()
    e.stopPropagation()
    const next = removePlatformDesktopTab(path)
    setTabs(next)
    if (platformDesktopTabActive(location.pathname, path)) {
      navigate(next[next.length - 1]?.path || '/platform')
    }
  }

  const popOutTab = (path: string, e: MouseEvent) => {
    e.preventDefault()
    e.stopPropagation()
    openCenterPopout(path)
  }

  if (tabs.length <= 1 && location.pathname === '/platform') return null

  return (
    <div className="mac-desktop-tabs flex items-center gap-1 px-2 sm:px-3 py-1 border-b border-white/[0.06] bg-black/20 overflow-x-auto shrink-0">
      {tabs.map((tab) => {
        const active = platformDesktopTabActive(location.pathname, tab.path)
        return (
          <Link
            key={tab.path}
            to={desktopTo(tab.path)}
            className={`mac-desktop-tab group flex items-center gap-1.5 rounded-lg px-2.5 py-1 text-xs whitespace-nowrap transition-colors ${
              active ? 'mac-desktop-tab-active text-[var(--text-primary)]' : 'text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--surface-hover)]'
            }`}
          >
            <span className="truncate max-w-[8rem]">{tab.label}</span>
            {tab.path !== '/platform' ? (
              <>
                <button
                  type="button"
                  onClick={(e) => popOutTab(tab.path, e)}
                  className="opacity-0 group-hover:opacity-100 hover:text-[var(--link)] transition-opacity"
                  title="Open in new window"
                  aria-label={`Pop out ${tab.label}`}
                >
                  <ExternalLink className="h-3 w-3" />
                </button>
                <button
                  type="button"
                  onClick={(e) => closeTab(tab.path, e)}
                  className={`opacity-0 group-hover:opacity-100 transition-opacity ${statusToneClass('error')}`}
                  aria-label={`Close ${tab.label}`}
                >
                  <X className="h-3 w-3" />
                </button>
              </>
            ) : null}
          </Link>
        )
      })}
      <button
        type="button"
        onClick={() => dispatchOpenSpotlight()}
        className="mac-desktop-tab-add flex items-center justify-center rounded-lg p-1 text-[var(--text-muted)] hover:text-[var(--text-primary)] hover:bg-[var(--surface-hover)]"
        title="Open center (⌘K)"
        aria-label="Open center"
      >
        <Plus className="h-3.5 w-3.5" />
      </button>
    </div>
  )
}

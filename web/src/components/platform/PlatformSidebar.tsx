// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { NavLink, useLocation } from 'react-router'
import { ChevronDown, ChevronLeft, ChevronRight, Boxes, FolderOpen, Plug, Server } from 'lucide-react'
import { ZyvorMark } from '../ZyvorMark'
import { sidebarForTier, sidebarLocationsOnly } from '../../utils/platformNavFilter'
import { integrationNavItems } from '../../utils/platformIntegrationsNav'
import { usePlatformInfo } from '../../contexts/PlatformInfoContext'
import { usePlatformDesktopTier } from '../../hooks/usePlatformDesktopTier'
import { usePlatformMacDesktop } from './mac/PlatformMacDesktopContext'
import { getFleetFinder } from '../../api/platform'
import type { PlatformNavSection } from '../../utils/platformNav'

const SECTION_COLLAPSE_PREFIX = 'machina-sidebar-section-'

const SECTION_ICONS: Record<string, typeof Plug> = {
  Host: Server,
  Fleet: Plug,
  Platform: FolderOpen,
  'Connected platforms': Boxes,
}

function sectionCollapseKey(label: string) {
  return `${SECTION_COLLAPSE_PREFIX}${label}`
}

function loadSectionCollapsed(section: PlatformNavSection): boolean {
  if (!section.collapsible) return false
  try {
    const raw = localStorage.getItem(sectionCollapseKey(section.label))
    if (raw === '0') return false
    if (raw === '1') return true
  } catch {
    /* ignore */
  }
  return section.defaultCollapsed ?? false
}

export default function PlatformSidebar() {
  const [tier] = usePlatformDesktopTier()
  const { info } = usePlatformInfo()
  const allSections = sidebarForTier(tier, integrationNavItems(info))
  // Menubar owns product menus; sidebar is the Finder locations rail (Host / Fleet / Platform).
  // Favorites only as expanded fallback on Normal — dock is retired.
  const locationSections = sidebarLocationsOnly(allSections)
  const sections = locationSections.length > 0 ? locationSections : allSections
  const favoritesOnlyRail = locationSections.length === 0
  const { sidebarCollapsed: collapsedState, setSidebarCollapsed: setCollapsed } = usePlatformMacDesktop()
  const collapsed = favoritesOnlyRail ? false : collapsedState
  const [sectionCollapsed, setSectionCollapsed] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(sections.map((s) => [s.label, loadSectionCollapsed(s)])),
  )
  const [railAttention, setRailAttention] = useState<Record<string, 'attention' | 'critical'>>({})

  useEffect(() => {
    void getFleetFinder()
      .then((f) => {
        const needs = f?.smart_folders.find((x) => x.id === 'needs_attention')?.count ?? 0
        const unprotected = f?.smart_folders.find((x) => x.id === 'unprotected')?.count ?? 0
        const next: Record<string, 'attention' | 'critical'> = {}
        if (needs > 0) next['/platform/vms'] = 'attention'
        if (unprotected > 0) next['/platform/backups'] = 'attention'
        setRailAttention(next)
      })
      .catch(() => setRailAttention({}))
  }, [])

  const toggleSection = useCallback((label: string) => {
    setSectionCollapsed((prev) => {
      const next = !prev[label]
      try {
        localStorage.setItem(sectionCollapseKey(label), next ? '1' : '0')
      } catch {
        /* ignore */
      }
      return { ...prev, [label]: next }
    })
  }, [])

  return (
    <>
      <aside
        className={`mac-finder-sidebar tahoe-sidebar tahoe-sidebar-expanded platform-sidebar glass glass-elevated hidden lg:flex flex-col shrink-0 border-r border-[var(--apple-hairline)] ${
          collapsed ? 'w-[60px]' : 'w-[280px]'
        }`}
        aria-label="Finder"
      >
        {!collapsed && (
          <div className="px-5 py-4 border-b border-[var(--apple-hairline)] space-y-2">
            <ZyvorMark to="/" size="sm" />
            <p className="text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">Desktop</p>
            <p className="text-sm text-[var(--text-secondary)]">Machina</p>
          </div>
        )}
        {collapsed && (
          <div className="flex justify-center py-3 border-b border-[var(--apple-hairline)]">
            <ZyvorMark to="/" size="sm" showWordmark={false} />
          </div>
        )}
        <SidebarNav
          collapsed={collapsed}
          sections={sections}
          sectionCollapsed={sectionCollapsed}
          onToggleSection={toggleSection}
          railAttention={railAttention}
        />
        <div className="border-t border-[var(--apple-hairline)] p-3">
          {favoritesOnlyRail ? (
            <p className="px-3 py-1 text-[10px] text-center text-[var(--text-muted)]">
              Apps live in the Dock — View → Hide Sidebar
            </p>
          ) : (
            <button
              type="button"
              onClick={() => setCollapsed(!collapsed)}
              className="flex w-full items-center justify-center gap-2 rounded-xl px-3 py-2.5 text-xs text-[var(--text-muted)] hover:bg-[var(--surface-hover,rgba(16,20,28,0.035))] hover:text-[var(--text-primary)] transition"
              title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
            >
              {collapsed ? <ChevronRight className="h-4 w-4" /> : (
                <>
                  <ChevronLeft className="h-4 w-4" />
                  <span>Collapse</span>
                </>
              )}
            </button>
          )}
        </div>
      </aside>
    </>
  )
}

function SidebarNav({
  collapsed,
  sections,
  sectionCollapsed,
  onToggleSection,
  railAttention,
}: {
  collapsed: boolean
  sections: ReturnType<typeof sidebarForTier>
  sectionCollapsed: Record<string, boolean>
  onToggleSection: (label: string) => void
  railAttention: Record<string, 'attention' | 'critical'>
}) {
  const location = useLocation()

  const railClassFor = (to: string, isActive: boolean) => {
    if (isActive) return 'platform-rail-active'
    if (railAttention[to] === 'critical') return 'platform-rail-critical'
    if (railAttention[to] === 'attention') return 'platform-rail-attention'
    if (to === '/platform' && location.pathname === '/platform') return 'platform-rail-live'
    return ''
  }
  return (
    <nav className="flex-1 py-3 px-3 space-y-2">
      {sections.map((section, sectionIdx) => {
        const isFavoritesZone = sectionIdx === 0 && section.label === 'Favorites'
        const isSectionClosed = section.collapsible && sectionCollapsed[section.label]
        const SectionIcon = SECTION_ICONS[section.label]

        return (
          <div key={section.label} className="tahoe-sidebar-section">
            {!collapsed && !isFavoritesZone && (
              section.collapsible ? (
                <button
                  type="button"
                  onClick={() => onToggleSection(section.label)}
                  className="tahoe-sidebar-section-header flex w-full items-center gap-1.5 px-3 py-2 mb-1 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)] hover:text-[var(--text-secondary)] transition"
                >
                  <ChevronDown
                    className={`h-3 w-3 shrink-0 transition-transform ${isSectionClosed ? '-rotate-90' : ''}`}
                  />
                  {SectionIcon ? <SectionIcon className="h-3 w-3 shrink-0 opacity-60" /> : null}
                  <span className="truncate">{section.label}</span>
                </button>
              ) : (
                <p className="tahoe-sidebar-section-header flex items-center gap-1.5 px-3 py-2 mb-1 text-[10px] font-semibold uppercase tracking-wider text-[var(--text-muted)]">
                  {SectionIcon ? <SectionIcon className="h-3 w-3 shrink-0 opacity-60" /> : null}
                  <span>{section.label}</span>
                </p>
              )
            )}
            {!isSectionClosed && (
              <ul className="space-y-1">
                {section.items.map((item) => (
                  <li key={item.to}>
                    <NavLink
                      to={item.to}
                      end={item.to === '/platform' || item.to === '/' || item.to === '/vms' || item.to === '/fleet-cloud'}
                      title={collapsed ? item.label : undefined}
                      className={({ isActive }) =>
                        `tahoe-sidebar-link platform-rail-link flex items-center gap-3 px-3 py-2.5 text-sm transition-all duration-200 ${
                          collapsed ? 'justify-center rounded-xl' : 'rounded-full'
                        } ${
                          isActive
                            ? 'tahoe-sidebar-link-active text-[var(--text-primary)]'
                            : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--surface-hover,rgba(255,255,255,0.04))]'
                        } ${railClassFor(item.to, isActive)}`
                      }
                    >
                      <span className="shrink-0 opacity-80">{item.icon}</span>
                      {!collapsed && <span className="truncate">{item.label}</span>}
                    </NavLink>
                  </li>
                ))}
              </ul>
            )}
            {!collapsed && sectionIdx < sections.length - 1 && (
              <div className="tahoe-sidebar-divider mx-3 my-3" aria-hidden />
            )}
          </div>
        )
      })}
    </nav>
  )
}

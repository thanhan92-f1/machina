// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useMemo, useState } from 'react'
import { NavLink, useLocation } from 'react-router'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { integrationNavItems } from '../../utils/platformIntegrationsNav'
import { usePlatformInfo } from '../../contexts/PlatformInfoContext'
import { usePlatformDesktopTier } from '../../hooks/usePlatformDesktopTier'
import { usePlatformMacDesktop } from './mac/PlatformMacDesktopContext'
import { getFleetFinder } from '../../api/platform'
import {
  sidebarProductSectionsForTier,
  sidebarRailPinnedForTier,
  type SidebarProductSection,
  type SidebarRailItem,
} from '../../utils/platformSidebarNav'
import { navItemActive } from '../../utils/routes'
import PlatformSidebarSection from './PlatformSidebarSection'

const SECTION_COLLAPSE_PREFIX = 'machina-sidebar-product-'

function loadSectionExpanded(sectionId: string): boolean {
  try {
    const raw = localStorage.getItem(`${SECTION_COLLAPSE_PREFIX}${sectionId}`)
    if (raw === '0') return false
    if (raw === '1') return true
  } catch {
    /* ignore */
  }
  return true
}

function SidebarLink({
  item,
  collapsed,
  isActive,
  attentionClass,
}: {
  item: SidebarRailItem
  collapsed: boolean
  isActive: boolean
  attentionClass: string
}) {
  const Icon = item.icon
  return (
    <NavLink
      to={item.to}
      end={item.to === '/platform' || item.to === '/' || item.to === '/vms' || item.to === '/fleet-cloud'}
      title={item.label}
      aria-label={item.label}
      className={`platform-sidebar-link tahoe-sidebar-link platform-rail-link flex items-center gap-2.5 text-sm transition-all duration-200 ${
        collapsed ? 'justify-center rounded-xl px-1.5 py-2' : 'rounded-full px-2.5 py-2'
      } ${
        isActive
          ? 'tahoe-sidebar-link-active text-[var(--text-primary)]'
          : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--surface-hover,rgba(255,255,255,0.04))]'
      } ${attentionClass}`}
    >
      <span className="platform-sidebar-icon shrink-0" aria-hidden>
        <Icon className="h-[1.15rem] w-[1.15rem]" strokeWidth={1.5} />
      </span>
      {collapsed ? <span className="sr-only">{item.label}</span> : <span className="truncate">{item.label}</span>}
    </NavLink>
  )
}

function FlyoutLink({ item, onNavigate }: { item: SidebarRailItem; onNavigate?: () => void }) {
  const location = useLocation()
  const Icon = item.icon
  const active = navItemActive({ to: item.to, label: item.label, icon: null }, location.pathname, location.search)
  return (
    <NavLink
      to={item.to}
      title={item.label}
      onClick={onNavigate}
      className={`platform-sidebar-flyout-link flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm transition ${
        active
          ? 'bg-[var(--surface-hover,rgba(255,255,255,0.08))] text-[var(--text-primary)] font-medium'
          : 'text-[var(--text-secondary)] hover:bg-[var(--surface-hover,rgba(255,255,255,0.06))] hover:text-[var(--text-primary)]'
      }`}
      role="menuitem"
    >
      <Icon className="h-4 w-4 shrink-0 opacity-80" strokeWidth={1.5} />
      <span className="truncate">{item.label}</span>
    </NavLink>
  )
}

export default function PlatformSidebar() {
  const [tier] = usePlatformDesktopTier()
  const { info } = usePlatformInfo()
  const integrations = integrationNavItems(info)
  const pinned = useMemo(() => sidebarRailPinnedForTier(tier), [tier])
  const productSections = useMemo(
    () => sidebarProductSectionsForTier(tier, integrations),
    [tier, integrations],
  )
  const { sidebarCollapsed: collapsed, setSidebarCollapsed: setCollapsed } = usePlatformMacDesktop()
  const location = useLocation()
  const [sectionExpanded, setSectionExpanded] = useState<Record<string, boolean>>(() =>
    Object.fromEntries(productSections.map((s) => [s.id, loadSectionExpanded(s.id)])),
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

  const toggleSection = useCallback((id: string) => {
    setSectionExpanded((prev) => {
      const next = !prev[id]
      try {
        localStorage.setItem(`${SECTION_COLLAPSE_PREFIX}${id}`, next ? '1' : '0')
      } catch {
        /* ignore */
      }
      return { ...prev, [id]: next }
    })
  }, [])

  const railClassFor = (to: string, isActive: boolean) => {
    if (isActive) return 'platform-rail-active'
    if (railAttention[to] === 'critical') return 'platform-rail-critical'
    if (railAttention[to] === 'attention') return 'platform-rail-attention'
    if (to === '/platform' && location.pathname === '/platform') return 'platform-rail-live'
    return ''
  }

  const sectionHasActive = (section: SidebarProductSection) =>
    section.items.some((item) =>
      navItemActive({ to: item.to, label: item.label, icon: null }, location.pathname, location.search),
    )

  return (
    <aside
      className={`mac-finder-sidebar tahoe-sidebar platform-sidebar platform-sidebar--zeus glass glass-elevated hidden lg:flex flex-col shrink-0 border-r border-[var(--apple-hairline)] ${
        collapsed ? 'platform-sidebar--rail w-[68px]' : 'platform-sidebar--expanded w-[15rem]'
      }`}
      aria-label="Navigation"
      data-collapsed={collapsed ? '1' : '0'}
    >
      <nav className="flex-1 min-h-0 overflow-y-auto overscroll-contain py-2 px-1.5 space-y-1" id="platform-primary-nav">
        <ul className="space-y-0.5">
          {pinned.map((item) => (
            <li key={item.to}>
              <SidebarLink
                item={item}
                collapsed={collapsed}
                isActive={navItemActive({ to: item.to, label: item.label, icon: null }, location.pathname, location.search)}
                attentionClass={railClassFor(
                  item.to,
                  navItemActive({ to: item.to, label: item.label, icon: null }, location.pathname, location.search),
                )}
              />
            </li>
          ))}
        </ul>

        {productSections.length > 0 ? (
          <div className="platform-sidebar-divider mx-1 my-2" aria-hidden />
        ) : null}

        {productSections.map((section) => (
          <PlatformSidebarSection
            key={section.id}
            label={section.label}
            icon={section.icon}
            collapsed={collapsed}
            expanded={sectionExpanded[section.id] ?? true}
            onToggleExpanded={() => toggleSection(section.id)}
            hasActiveItem={sectionHasActive(section)}
            flyoutItems={section.items.map((item) => (
              <FlyoutLink key={item.to} item={item} />
            ))}
          >
            <ul className="space-y-0.5 mb-1">
              {section.items.map((item) => (
                <li key={item.to}>
                  <SidebarLink
                    item={item}
                    collapsed={false}
                    isActive={navItemActive({ to: item.to, label: item.label, icon: null }, location.pathname, location.search)}
                    attentionClass={railClassFor(
                      item.to,
                      navItemActive({ to: item.to, label: item.label, icon: null }, location.pathname, location.search),
                    )}
                  />
                </li>
              ))}
            </ul>
          </PlatformSidebarSection>
        ))}
      </nav>

      <div className="border-t border-[var(--apple-hairline)] p-2">
        <button
          type="button"
          onClick={() => setCollapsed(!collapsed)}
          className="flex w-full items-center justify-center rounded-xl p-2 text-[var(--text-muted)] hover:bg-[var(--surface-hover,rgba(255,255,255,0.04))] hover:text-[var(--text-primary)] transition"
          title={collapsed ? 'Expand sidebar' : 'Icon rail only'}
          aria-label={collapsed ? 'Expand sidebar' : 'Icon rail only'}
        >
          {collapsed ? <ChevronRight className="h-4 w-4" /> : <ChevronLeft className="h-4 w-4" />}
        </button>
      </div>
    </aside>
  )
}

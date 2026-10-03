// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { NavLink, useLocation } from 'react-router'
import { ChevronsLeft, ChevronsRight, Compass, LayoutGrid, Settings, type LucideIcon } from 'lucide-react'
import { getFleetFinder } from '../../api/platform'
import { usePlatformInfo } from '../../contexts/PlatformInfoContext'
import { usePlatformDesktopTier } from '../../hooks/usePlatformDesktopTier'
import { integrationNavItems } from '../../utils/platformIntegrationsNav'
import {
  sidebarProductSectionsForTier,
  type SidebarProductSection,
  type SidebarRailItem,
} from '../../utils/platformSidebarNav'
import { navItemActive } from '../../utils/routes'
import PlatformSidebarSection from '../platform/PlatformSidebarSection'
import { usePlatformMacDesktop } from '../platform/mac/PlatformMacDesktopContext'

// The top-nav mega-menu covers every route; this sidebar is a fast path (only the section you are
// in is open by default; the rest are one-line headers) to the categories people use most — Workloads/Infra/Ops/Secure — pulled from
// the same live registry (sidebarProductSectionsForTier) so it never drifts out of
// sync with the GlobalBar flyouts. Admin/More stay top-nav-only to keep this list
// from growing unbounded.
const RAIL_SECTION_IDS = ['workloads', 'infra', 'ops', 'secure']

const OVERVIEW: SidebarRailItem[] = [
  { to: '/platform', label: 'Mission Control', icon: LayoutGrid },
  { to: '/platform/hosts/finder', label: 'Machine Finder', icon: Compass },
]

const SECTION_COLLAPSE_PREFIX = 'machina-sidenav-'

/** Explicit user choice for a section, or undefined to follow the active route. */
function loadSectionExpanded(id: string): boolean | undefined {
  try {
    const raw = localStorage.getItem(`${SECTION_COLLAPSE_PREFIX}${id}`)
    if (raw === '0') return false
    if (raw === '1') return true
  } catch {
    /* ignore */
  }
  return undefined
}

function SideLink({
  item,
  isActive,
  badge,
  end,
}: {
  item: SidebarRailItem
  isActive: boolean
  badge?: number
  end?: boolean
}) {
  const Icon = item.icon
  return (
    <NavLink
      to={item.to}
      end={end}
      title={item.label}
      aria-label={item.label}
      className={`gnb-side-link flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm transition-all duration-200 ${
        isActive
          ? 'gnb-side-link-active text-[var(--text-primary)]'
          : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--surface-hover,rgba(255,255,255,0.04))]'
      }`}
    >
      <span className="gnb-side-icon relative shrink-0" aria-hidden>
        <Icon className="h-[1.15rem] w-[1.15rem]" strokeWidth={1.5} />
        {badge ? <i className="gnb-side-dot" /> : null}
      </span>
      <span className="truncate flex-1">{item.label}</span>
      {badge ? <span className="gnb-side-count">{badge}</span> : null}
    </NavLink>
  )
}

export default function SideNav({ mobileOpen, onCloseMobile }: { mobileOpen: boolean; onCloseMobile: () => void }) {
  const location = useLocation()
  const { sidebarCollapsed, setSidebarCollapsed } = usePlatformMacDesktop()
  // Icon rail on desktop; the mobile drawer is always the full-width list.
  const rail = sidebarCollapsed && !mobileOpen
  const [tier] = usePlatformDesktopTier()
  const { info } = usePlatformInfo()
  const integrations = integrationNavItems(info)
  const allSections = useMemo(
    () => sidebarProductSectionsForTier(tier, integrations),
    [tier, integrations],
  )
  const sections = useMemo(
    () => allSections.filter((s) => RAIL_SECTION_IDS.includes(s.id)),
    [allSections],
  )
  const [sectionExpanded, setSectionExpanded] = useState<Record<string, boolean | undefined>>(() =>
    Object.fromEntries(sections.map((s) => [s.id, loadSectionExpanded(s.id)])),
  )
  const [vmsNeedAttention, setVmsNeedAttention] = useState(0)

  useEffect(() => {
    void getFleetFinder()
      .then((f) => {
        const needs = f?.smart_folders.find((x) => x.id === 'needs_attention')?.count ?? 0
        setVmsNeedAttention(needs)
      })
      .catch(() => setVmsNeedAttention(0))
  }, [])

  // Skip the first run: this effect closes the drawer on navigation, but SideNav can now mount
  // *because* the mobile drawer just opened (PlatformLayout renders it on `mobileNavOpen` even
  // when the desktop "hide sidebar" preference is on) — without this guard, that initial mount
  // immediately called onCloseMobile() and closed the drawer it had just opened.
  const skippedFirstPathEffect = useRef(false)
  useEffect(() => {
    if (!skippedFirstPathEffect.current) {
      skippedFirstPathEffect.current = true
      return
    }
    onCloseMobile()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [location.pathname])

  // `currentlyOpen` is what the user sees now (explicit choice, else "is the active route in it"),
  // so the first click always flips the visible state.
  const toggleSection = useCallback((id: string, currentlyOpen: boolean) => {
    setSectionExpanded((prev) => {
      const next = !currentlyOpen
      try {
        localStorage.setItem(`${SECTION_COLLAPSE_PREFIX}${id}`, next ? '1' : '0')
      } catch {
        /* ignore */
      }
      return { ...prev, [id]: next }
    })
  }, [])

  const badgeFor = (to: string) => (to === '/platform/vms' ? vmsNeedAttention : 0)

  return (
    <>
      <aside
        className={`gnb-side ${rail ? 'gnb-side--rail' : 'gnb-side--expanded'} flex flex-col shrink-0 border-r border-[var(--apple-hairline)] ${mobileOpen ? 'gnb-side-mobile-open' : ''}`}
        aria-label="Sections"
      >
        <SideNavBody
          sections={sections}
          sectionExpanded={sectionExpanded}
          toggleSection={toggleSection}
          badgeFor={badgeFor}
          location={location}
          rail={rail}
        />
        <div className="gnb-side-footer border-t border-[var(--apple-hairline)] p-2">
          <NavLink to="/platform/settings" title="Settings" aria-label="Settings" className="gnb-side-link flex items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm text-[var(--text-secondary)]">
            <Settings className="h-[1.15rem] w-[1.15rem] shrink-0" strokeWidth={1.5} />
            <span>Settings</span>
          </NavLink>
          <button
            type="button"
            className="gnb-side-link gnb-side-toggle hidden lg:flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm text-[var(--text-secondary)] hover:text-[var(--text-primary)]"
            onClick={() => setSidebarCollapsed(!sidebarCollapsed)}
            aria-pressed={sidebarCollapsed}
            aria-label={sidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar to icons'}
            title={sidebarCollapsed ? 'Expand sidebar' : 'Collapse to icons'}
          >
            {sidebarCollapsed ? (
              <ChevronsRight className="h-[1.15rem] w-[1.15rem] shrink-0" strokeWidth={1.5} />
            ) : (
              <ChevronsLeft className="h-[1.15rem] w-[1.15rem] shrink-0" strokeWidth={1.5} />
            )}
            <span>Collapse</span>
          </button>
        </div>
      </aside>
      {mobileOpen ? <div className="gnb-scrim gnb-scrim-mobile-only" onClick={onCloseMobile} /> : null}
    </>
  )
}

function SideNavBody({
  sections,
  sectionExpanded,
  toggleSection,
  badgeFor,
  location,
  rail,
}: {
  sections: SidebarProductSection[]
  sectionExpanded: Record<string, boolean | undefined>
  toggleSection: (id: string, currentlyOpen: boolean) => void
  badgeFor: (to: string) => number
  location: ReturnType<typeof useLocation>
  rail: boolean
}) {
  const sectionHasActive = (section: SidebarProductSection) =>
    section.items.some((item) =>
      navItemActive({ to: item.to, label: item.label, icon: null }, location.pathname, location.search),
    )

  return (
    <nav className="flex-1 min-h-0 overflow-y-auto overscroll-contain py-2 px-1.5 space-y-1">
      <ul className="space-y-0.5">
        {OVERVIEW.map((item) => (
          <li key={item.to}>
            <SideLink
              item={item}
              end={item.to === '/platform'}
              isActive={navItemActive({ to: item.to, label: item.label, icon: null }, location.pathname, location.search)}
            />
          </li>
        ))}
      </ul>

      <div className="gnb-side-divider mx-1 my-2" aria-hidden />

      {sections.map((section: SidebarProductSection) => {
        const open = sectionExpanded[section.id] ?? sectionHasActive(section)
        return (
        <PlatformSidebarSection
          key={section.id}
          label={section.label}
          icon={section.icon as LucideIcon}
          collapsed={rail}
          expanded={open}
          onToggleExpanded={() => toggleSection(section.id, open)}
          hasActiveItem={sectionHasActive(section)}
          flyoutItems={section.items.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              role="menuitem"
              className={`flex items-center gap-2.5 rounded-lg px-3 py-2 text-sm ${
                navItemActive({ to: item.to, label: item.label, icon: null }, location.pathname, location.search)
                  ? 'font-medium text-[var(--text-primary)] bg-[var(--nl-fill,rgba(0,0,0,0.04))]'
                  : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--surface-hover,rgba(0,0,0,0.04))]'
              }`}
            >
              <item.icon className="h-4 w-4 shrink-0" strokeWidth={1.5} />
              <span className="truncate">{item.label}</span>
            </NavLink>
          ))}
        >
          <ul className="space-y-0.5 mb-2">
            {section.items.map((item) => (
              <li key={item.to}>
                <SideLink
                  item={item}
                  isActive={navItemActive({ to: item.to, label: item.label, icon: null }, location.pathname, location.search)}
                  badge={badgeFor(item.to)}
                />
              </li>
            ))}
          </ul>
        </PlatformSidebarSection>
        )
      })}
    </nav>
  )
}

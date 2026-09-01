// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import type { ReactNode } from 'react'
import type { PlatformDesktopTier } from './platformDesktopTier'
import { isPathAllowedForTier } from './platformDesktopTier'
import { DESKTOP_HUB_TILES } from './platformHubZones'
import { hubHrefForTier } from './platformHubLinks'
import { sidebarForTier, sidebarLocationsOnly } from './platformNavFilter'
import type { PlatformNavItem, PlatformNavSection } from './platformNav'

export type MacMenuNavItem = { to: string; label: string }

function dedupeItems(items: PlatformNavItem[], seen: Set<string>): PlatformNavItem[] {
  const next: PlatformNavItem[] = []
  for (const item of items) {
    if (seen.has(item.to)) continue
    seen.add(item.to)
    next.push(item)
  }
  return next
}

/** Hub-first Go menu — favorites, hubs, then tier-filtered Host / Fleet / Platform locations. */
export function macMenuSectionsForTier(tier: PlatformDesktopTier, integrationItems: PlatformNavItem[] = []): PlatformNavSection[] {
  const seenPaths = new Set<string>()

  const favorites = dedupeItems(
    sidebarForTier('normal', [])
      .flatMap((section) => section.items)
      .filter((item) => isPathAllowedForTier(item.to, tier)),
    seenPaths,
  )

  const hubItems = dedupeItems(
    DESKTOP_HUB_TILES
      .filter((hub) => isPathAllowedForTier(hub.href, tier))
      .map((hub) => ({
        to: hubHrefForTier(hub.id, tier),
        label: hub.label,
        icon: null as ReactNode,
      })),
    seenPaths,
  )

  const sections: PlatformNavSection[] = []
  if (favorites.length > 0) {
    sections.push({ label: 'Favorites', items: favorites })
  }
  if (hubItems.length > 0) {
    sections.push({ label: 'Hubs', items: hubItems })
  }

  const locationSections = sidebarLocationsOnly(sidebarForTier(tier, integrationItems))
    .map((section) => ({
      ...section,
      items: dedupeItems(
        section.items.filter((item) => isPathAllowedForTier(item.to, tier)),
        seenPaths,
      ),
    }))
    .filter((section) => section.items.length > 0)

  sections.push(...locationSections)
  return sections
}

export function flattenMacMenuForTier(tier: PlatformDesktopTier, integrationItems: PlatformNavItem[] = []): MacMenuNavItem[] {
  return macMenuSectionsForTier(tier, integrationItems).flatMap((s) =>
    s.items.map((item) => ({ to: item.to, label: item.label })),
  )
}

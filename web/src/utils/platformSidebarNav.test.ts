// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { describe, expect, it } from 'vitest'
import { sidebarProductSectionsForTier, sidebarRailPinnedForTier } from './platformSidebarNav'

describe('sidebarRailPinnedForTier', () => {
  it('returns Zeus-style pinned destinations with icons', () => {
    const pinned = sidebarRailPinnedForTier('advanced')
    expect(pinned.map((i) => i.to)).toEqual([
      '/platform',
      '/vms',
      '/platform/vms',
      '/platform/hosts',
      '/platform/settings',
    ])
    expect(pinned.every((i) => i.label && i.icon)).toBe(true)
  })
})

describe('sidebarProductSectionsForTier', () => {
  it('mirrors menubar product groups for the sidebar', () => {
    const sections = sidebarProductSectionsForTier('normal')
    expect(sections.map((s) => s.id)).toEqual(
      expect.arrayContaining(['workloads', 'infra', 'ops', 'secure', 'admin', 'more']),
    )
    const flat = sections.flatMap((s) => s.items)
    expect(flat.some((i) => i.to === '/platform/vms')).toBe(true)
    expect(flat.every((i) => i.icon)).toBe(true)
  })
})

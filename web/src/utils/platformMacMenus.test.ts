// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { describe, expect, it } from 'vitest'
import { macMenuSectionsForTier } from './platformMacMenus'

describe('macMenuSectionsForTier', () => {
  it('normal tier includes favorites without Host locations', () => {
    const sections = macMenuSectionsForTier('normal')
    const labels = sections.map((s) => s.label)
    expect(labels).toContain('Favorites')
    expect(labels).not.toContain('Host')
    expect(labels).not.toContain('Fleet')
    expect(labels).not.toContain('Platform')
  })

  it('advanced tier appends Host locations after hubs', () => {
    const sections = macMenuSectionsForTier('advanced')
    const labels = sections.map((s) => s.label)
    expect(labels).toContain('Favorites')
    expect(labels).toContain('Hubs')
    expect(labels).toContain('Host')

    const host = sections.find((s) => s.label === 'Host')
    expect(host?.items.some((item) => item.to === '/networks')).toBe(true)
  })

  it('dedupes hub paths already listed in favorites', () => {
    const sections = macMenuSectionsForTier('power')
    const paths = sections.flatMap((s) => s.items.map((i) => i.to))
    expect(new Set(paths).size).toBe(paths.length)
  })
})

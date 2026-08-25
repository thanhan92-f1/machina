// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import type { LaunchpadApp } from '../api/launchpad'

export type LaunchpadSpaceId =
  | 'monitoring'
  | 'virtualization'
  | 'security'
  | 'storage'
  | 'developer'
  | 'ai'
  | 'other'

export interface LaunchpadSpace {
  id: LaunchpadSpaceId
  label: string
  description: string
  matchCategories: string[]
}

export const LAUNCHPAD_SPACES: LaunchpadSpace[] = [
  {
    id: 'monitoring',
    label: 'Monitoring',
    description: 'Dashboards, metrics, and observability',
    matchCategories: ['monitoring', 'observability'],
  },
  {
    id: 'virtualization',
    label: 'Virtualization',
    description: 'VM consoles and virtualization tools',
    matchCategories: ['virtualization', 'vm', 'console'],
  },
  {
    id: 'security',
    label: 'Security',
    description: 'Identity, firewall, and security tooling',
    matchCategories: ['security', 'identity'],
  },
  {
    id: 'storage',
    label: 'Storage',
    description: 'Volumes, object stores, and backups',
    matchCategories: ['storage'],
  },
  {
    id: 'developer',
    label: 'Developer',
    description: 'CI/CD, GitOps, and dev tools',
    matchCategories: ['ci/cd', 'gitops', 'developer tools'],
  },
  {
    id: 'ai',
    label: 'AI',
    description: 'AI workloads and assistants',
    matchCategories: ['ai'],
  },
  {
    id: 'other',
    label: 'Other',
    description: 'Everything else in the catalog',
    matchCategories: [],
  },
]

export function launchpadSpaceForCategory(category: string): LaunchpadSpaceId {
  const cat = category.trim().toLowerCase()
  for (const space of LAUNCHPAD_SPACES) {
    if (space.id === 'other') continue
    if (space.matchCategories.some((c) => cat.includes(c) || c.includes(cat))) {
      return space.id
    }
  }
  return 'other'
}

export function groupAppsBySpace(apps: LaunchpadApp[]): Map<LaunchpadSpaceId, LaunchpadApp[]> {
  const map = new Map<LaunchpadSpaceId, LaunchpadApp[]>()
  for (const space of LAUNCHPAD_SPACES) {
    map.set(space.id, [])
  }
  for (const app of apps) {
    const spaceId = launchpadSpaceForCategory(app.category || 'Custom')
    map.get(spaceId)?.push(app)
  }
  return map
}

export function spaceCounts(apps: LaunchpadApp[]): Record<LaunchpadSpaceId, number> {
  const grouped = groupAppsBySpace(apps)
  const counts = {} as Record<LaunchpadSpaceId, number>
  for (const space of LAUNCHPAD_SPACES) {
    counts[space.id] = grouped.get(space.id)?.length ?? 0
  }
  return counts
}

export function launchpadSpaceById(id: string): LaunchpadSpace | undefined {
  return LAUNCHPAD_SPACES.find((s) => s.id === id)
}

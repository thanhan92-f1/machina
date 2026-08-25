// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { platformFetch } from './platform'

export interface LaunchpadBackend {
  kind: string
  name: string
  port: number
  scheme: string
  path: string
}

export interface LaunchpadVisibility {
  published: boolean
  hidden: boolean
  favorite: boolean
}

export interface LaunchpadApp {
  id: string
  slug: string
  canonicalSlug?: string
  displayName: string
  description?: string
  namespace: string
  category: string
  icon: string
  backend: LaunchpadBackend
  routePath: string
  publicUrl: string
  status: string
  statusMessage?: string
  source: string
  authMode: string
  score: number
  visibility: LaunchpadVisibility
  readyEndpoints: number
  updatedAt: string
  meta?: {
    environment?: string
    owner?: string
    dependsOn?: string[]
    recommended?: boolean
  }
}

export interface LaunchpadSearchHit {
  app: LaunchpadApp
  score: number
}

export interface LaunchpadHealthSummary {
  total: number
  healthy: number
  degraded: number
  broken: number
  apps: LaunchpadApp[]
}

export interface LaunchpadConfig {
  publicBase: string
  pathPrefix: string
  enabled: boolean
}

let cachedConfig: LaunchpadConfig | null = null

export async function getLaunchpadConfig(): Promise<LaunchpadConfig> {
  if (cachedConfig) return cachedConfig
  const cfg = await platformFetch<LaunchpadConfig>('/api/v1/launchpad/config')
  cachedConfig = {
    publicBase: cfg.publicBase ?? '',
    pathPrefix: cfg.pathPrefix ?? '/launchpad',
    enabled: cfg.enabled ?? true,
  }
  return cachedConfig
}

/** Synchronously read the last-fetched launchpad config, or null before it has loaded. */
export function getCachedLaunchpadConfig(): LaunchpadConfig | null {
  return cachedConfig
}

export interface LaunchpadDiagnosisChainNode {
  id: string
  label: string
  status?: string
}

export interface LaunchpadSuggestedAction {
  label: string
  href: string
}

export interface LaunchpadDiagnosis {
  appId: string
  routePath: string
  publicUrl: string
  backend: LaunchpadBackend
  problem?: string
  cause?: string
  chain: LaunchpadDiagnosisChainNode[]
  suggestedActions: LaunchpadSuggestedAction[]
}

function launchpadAppPath(id: string, suffix = '') {
  const encoded = id.split('/').map(encodeURIComponent).join('/')
  return `/api/v1/launchpad/apps/${encoded}${suffix}`
}

export const listLaunchpadApps = () => platformFetch<LaunchpadApp[]>('/api/v1/launchpad/apps')
export const listLaunchpadCatalog = () => platformFetch<LaunchpadApp[]>('/api/v1/launchpad/catalog')
export const listLaunchpadFavorites = async (): Promise<LaunchpadApp[]> => {
  try {
    return await platformFetch<LaunchpadApp[]>('/api/v1/launchpad/favorites')
  } catch {
    // Hermes may return HTML 404 when the favorites route is missing — treat as empty dock.
    return []
  }
}
export const getLaunchpadApp = (id: string) => platformFetch<LaunchpadApp>(launchpadAppPath(id))
export const getLaunchpadDiagnosis = (id: string) =>
  platformFetch<LaunchpadDiagnosis>(launchpadAppPath(id, '/diagnosis'))
export const pinLaunchpadFavorite = (id: string) =>
  platformFetch<void>(`/api/v1/launchpad/favorites/${id.split('/').map(encodeURIComponent).join('/')}`, {
    method: 'PUT',
  })
export const launchpadHealthSummary = () => platformFetch<LaunchpadHealthSummary>('/api/v1/launchpad/health/apps')
export const searchLaunchpad = (q: string) =>
  platformFetch<LaunchpadSearchHit[]>(`/api/v1/launchpad/search?q=${encodeURIComponent(q)}&limit=20`)

// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

// Native compute flavor catalog (controller::api::flavors) — Phase 1 of the
// OpenStack-client replacement. Goes through the platform controller proxy, not the
// daemon's external-OpenStack-client routes in api/openstack.ts.

import { platformFetch } from './platform'

export interface NativeFlavor {
  id: string
  name: string
  vcpus: number
  memory_mib: number
  disk_gib: number
  description: string
  is_public: boolean
}

export function listFlavors(): Promise<NativeFlavor[]> {
  return platformFetch<NativeFlavor[]>('/api/v1/flavors')
}

export function getFlavor(id: string): Promise<NativeFlavor> {
  return platformFetch<NativeFlavor>(`/api/v1/flavors/${encodeURIComponent(id)}`)
}

export function createFlavor(body: {
  name: string
  vcpus: number
  memory_mib: number
  disk_gib: number
  description?: string
  is_public?: boolean
}): Promise<NativeFlavor> {
  return platformFetch<NativeFlavor>('/api/v1/flavors', {
    method: 'POST',
    body: JSON.stringify(body),
  })
}

export async function deleteFlavor(id: string): Promise<void> {
  await platformFetch<{ deleted: boolean }>(`/api/v1/flavors/${encodeURIComponent(id)}`, {
    method: 'DELETE',
  })
}

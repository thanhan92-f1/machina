// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

// Native libvirt network catalog (controller::api::networks) — used for the
// network picker on native "instance" NIC attach. Goes through the platform
// controller proxy.

import { platformFetch } from './platform'

export interface NativeNetwork {
  id: string
  name: string
  backend: string
  vlan_id: number | null
  bridge: string | null
  segment_id: string | null
}

export function listNetworks(): Promise<NativeNetwork[]> {
  return platformFetch<NativeNetwork[]>('/api/v1/networks')
}

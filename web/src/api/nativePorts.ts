// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

// Native ports (controller::api::networking) — VM NIC bindings, the Neutron-port
// equivalent. Goes through the platform controller proxy.

import { platformFetch } from './platform'

export interface NativePort {
  id: string
  network_id: string
  project_id: string | null
  vm_id: string | null
  mac_address: string | null
  security_group_id: string | null
  status: string
}

export function listPorts(): Promise<NativePort[]> {
  return platformFetch<NativePort[]>('/api/v1/ports')
}

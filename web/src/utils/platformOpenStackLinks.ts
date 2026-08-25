// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { listOpenStackInstances, listOpenStackNetworks, type OpenStackInstance, type OpenStackNetwork } from '../api/openstack'

export async function findOpenStackInstanceForVm(
  vmName: string,
  vmId?: string,
): Promise<OpenStackInstance | null> {
  try {
    const res = await listOpenStackInstances({ search: vmName, limit: 20 })
    const exact = res.instances.find((i) => {
      if (i.name === vmName) return true
      if (vmId && i.metadata?.machina_vm_id === vmId) return true
      if (vmId && i.metadata?.vm_id === vmId) return true
      return false
    })
    return exact ?? res.instances[0] ?? null
  } catch {
    return null
  }
}

export async function findOpenStackNetworkByName(name: string): Promise<OpenStackNetwork | null> {
  try {
    const res = await listOpenStackNetworks()
    return res.networks.find((n) => n.name === name) ?? null
  } catch {
    return null
  }
}

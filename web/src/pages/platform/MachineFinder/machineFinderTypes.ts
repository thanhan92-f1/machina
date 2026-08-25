// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

export type MachineFinderLens =
  | 'grid'
  | 'gallery'
  | 'table'
  | 'topology'
  | 'timeline'
  | 'heatmap'
  | 'migration'

export type MachineFinderOverlay =
  | 'default'
  | 'health'
  | 'backup'
  | 'network'
  | 'security'
  | 'gpu'
  | 'cost'
  | 'migration'

export const CLIENT_ONLY_FOLDERS = new Set(['guest-gaps', 'guest_agent_missing'])

export const SOURCE_LABELS: Record<string, string> = {
  libvirt: 'Libvirt',
  kubevirt: 'KubeVirt',
  vmware: 'VMware',
  vsphere: 'VMware',
  proxmox: 'Proxmox',
  openstack: 'OpenStack',
  discovered: 'Discovered',
}

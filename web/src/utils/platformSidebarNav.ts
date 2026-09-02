// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import type { LucideIcon } from 'lucide-react'
import {
  Activity,
  BrainCircuit,
  Cloud,
  Cpu,
  HardDrive,
  Home,
  LayoutDashboard,
  LayoutGrid,
  Monitor,
  Network,
  Package,
  Server,
  Settings,
  Shield,
  Wrench,
} from 'lucide-react'
import type { PlatformDesktopTier } from './platformDesktopTier'
import { isPathAllowedForTier } from './platformDesktopTier'
import { menubarProductGroupsForTier } from './platformMacMenus'
import { PLATFORM_PAGE_LABELS, type PlatformNavItem } from './platformNav'

export type SidebarRailItem = {
  to: string
  label: string
  icon: LucideIcon
}

export type SidebarProductSection = {
  id: string
  label: string
  icon: LucideIcon
  items: SidebarRailItem[]
}

const GROUP_ICONS: Record<string, LucideIcon> = {
  workloads: Monitor,
  infra: Server,
  ops: Activity,
  secure: Shield,
  admin: Settings,
  more: LayoutGrid,
}

const RAIL_PINNED_PATHS = [
  '/platform',
  '/vms',
  '/platform/vms',
  '/platform/hosts',
  '/platform/settings',
] as const

const PATH_ICON_OVERRIDES: Record<string, LucideIcon> = {
  '/': Home,
  '/platform': LayoutDashboard,
  '/vms': Monitor,
  '/create': Cpu,
  '/containers': Package,
  '/networks': Network,
  '/storage': HardDrive,
  '/fleet-cloud': Cloud,
  '/platform/vms': Monitor,
  '/platform/hosts': Server,
  '/platform/settings': Settings,
  '/platform/infrastructure': Server,
  '/platform/workloads': Package,
  '/platform/operations': Wrench,
  '/platform/administration': Settings,
  '/platform/datacenter': Server,
  '/platform/storage': HardDrive,
  '/platform/networks': Network,
  '/platform/content': HardDrive,
  '/platform/templates': Package,
  '/platform/cloud-init': Cpu,
  '/platform/gpu': Cpu,
  '/platform/activity': Activity,
  '/platform/events': Activity,
  '/platform/backups': HardDrive,
  '/platform/maintenance': Wrench,
  '/platform/tasks': Activity,
  '/platform/notifications': Activity,
  '/platform/migration': Wrench,
  '/platform/fleet-snapshots': HardDrive,
  '/platform/placement': Shield,
  '/platform/reports': Activity,
  '/platform/topology': Network,
  '/platform/observability': Activity,
  '/platform/recommendations': Activity,
  '/platform/blueprints': LayoutGrid,
  '/platform/zyra': BrainCircuit,
  '/platform/zyra/configure': BrainCircuit,
  '/platform/ai-providers': BrainCircuit,
  '/platform/enroll': Server,
  '/platform/soc': Shield,
  '/platform/zeus/security': Shield,
  '/platform/zeus/security/firewall': Shield,
  '/platform/zeus/security/policies': Shield,
  '/platform/users': Settings,
  '/platform/projects': LayoutGrid,
  '/platform/policy': Shield,
  '/platform/enterprise': Settings,
  '/platform/api-keys': Settings,
  '/platform/webhooks': Settings,
  '/platform/applications': Package,
  '/platform/developer': LayoutGrid,
}

function labelForPath(path: string): string {
  const key = path.split('?')[0] || path
  return PLATFORM_PAGE_LABELS[key] ?? key.split('/').filter(Boolean).pop()?.replace(/-/g, ' ') ?? path
}

function iconForPath(path: string, fallback: LucideIcon): LucideIcon {
  const key = path.split('?')[0] || path
  return PATH_ICON_OVERRIDES[key] ?? fallback
}

/** Zeus-style pinned icon rail — primary day-to-day destinations. */
export function sidebarRailPinnedForTier(tier: PlatformDesktopTier): SidebarRailItem[] {
  return RAIL_PINNED_PATHS.filter((path) => isPathAllowedForTier(path, tier)).map((to) => ({
    to,
    label: labelForPath(to),
    icon: iconForPath(to, LayoutGrid),
  }))
}

/** Product groups moved from the menubar into the sidebar (Workloads, Infra, …). */
export function sidebarProductSectionsForTier(
  tier: PlatformDesktopTier,
  integrationItems: PlatformNavItem[] = [],
): SidebarProductSection[] {
  return menubarProductGroupsForTier(tier, integrationItems)
    .map((group) => ({
      id: group.id,
      label: group.compact,
      icon: GROUP_ICONS[group.id] ?? LayoutGrid,
      items: group.sections.flatMap((section) =>
        section.items.map((item) => ({
          to: item.to,
          label: item.label,
          icon: iconForPath(item.to, GROUP_ICONS[group.id] ?? LayoutGrid),
        })),
      ),
    }))
    .filter((section) => section.items.length > 0)
}


import type { LucideIcon } from 'lucide-react'
import { Activity, Bell, FileBarChart, GitBranch, HardDrive, LayoutDashboard, Monitor, Network, Server, Settings, Sparkles, Terminal, Download, Wrench, ShieldAlert } from 'lucide-react'
import { PLATFORM_SIDEBAR } from './platformNav'
import { DOCK_PATHS_BY_TIER, loadPlatformDesktopTier, type PlatformDesktopTier } from './platformDesktopTier'
import { dockPreviewPathsForTier } from './platformHubZones'

export type PlatformDockItem = {
  path: string
  label: string
  icon: LucideIcon
  /** Normal-tier preview pin — unlocks at Power user. */
  preview?: boolean
}

const DOCK_KEY = 'machina-platform-dock-pins'
export const PLATFORM_DOCK_CHANGED_EVENT = 'machina-platform-dock-changed'

const ICON_BY_PATH: Record<string, LucideIcon> = {
  '/platform': LayoutDashboard,
  '/platform/vms': Monitor,
  '/platform/hosts': Server,
  '/platform/storage': HardDrive,
  '/platform/networks': Network,
  '/platform/events': Terminal,
  '/platform/activity': Activity,
  '/platform/reports': FileBarChart,
  '/platform/topology': GitBranch,
  '/platform/maintenance': Download,
  '/platform/notifications': Bell,
  '/platform/zyra': Sparkles,
  '/platform/infrastructure': Server,
  '/platform/workloads': Monitor,
  '/platform/administration': Settings,
  '/platform/resources': Server,
  '/platform/operations': Wrench,
  '/platform/zeus/security': ShieldAlert,
  '/platform/settings': Settings,
}

const LABEL_BY_PATH: Record<string, string> = {
  '/platform': 'Mission Control',
  '/platform/hosts': 'Hosts',
  '/platform/vms': 'Machines',
  '/platform/storage': 'Storage',
  '/platform/networks': 'Network',
  '/platform/zyra': 'Zyra',
  '/platform/events': 'Terminal',
  '/platform/activity': 'Activity',
  '/platform/reports': 'Reports',
  '/platform/topology': 'Topology',
  '/platform/maintenance': 'Updates',
  '/platform/notifications': 'Alerts',
  '/platform/infrastructure': 'Infrastructure',
  '/platform/workloads': 'Workloads',
  '/platform/administration': 'Admin',
  '/platform/resources': 'Infrastructure',
  '/platform/operations': 'Ops',
  '/platform/zeus/security': 'Security',
}

/** Default pinned apps for the Machina platform dock (v9s MacDock pattern). */
export function defaultDockPathsForTier(tier: PlatformDesktopTier = loadPlatformDesktopTier()): string[] {
  return DOCK_PATHS_BY_TIER[tier]
}

export const PLATFORM_SIDEBAR_FLAT = PLATFORM_SIDEBAR.flatMap((s) =>
  s.items.map((item) => ({ path: item.to, label: item.label })),
)

function itemForPath(path: string, preview = false): PlatformDockItem | null {
  const flat = PLATFORM_SIDEBAR_FLAT.find((i) => i.path === path)
  const Icon = ICON_BY_PATH[path] ?? Monitor
  const label = LABEL_BY_PATH[path] ?? flat?.label
  if (label) return { path, label, icon: Icon, preview }
  if (path in ICON_BY_PATH) {
    return { path, label: path.split('/').pop() ?? path, icon: Icon, preview }
  }
  return null
}

function appendPreviewItems(items: PlatformDockItem[], tier: PlatformDesktopTier): PlatformDockItem[] {
  const previews = dockPreviewPathsForTier(tier)
    .filter((path) => !items.some((item) => item.path === path))
    .map((path) => itemForPath(path, true))
    .filter(Boolean) as PlatformDockItem[]
  return [...items, ...previews]
}

export function savePlatformDockPaths(paths: string[]) {
  localStorage.setItem(DOCK_KEY, JSON.stringify(paths))
  window.dispatchEvent(new CustomEvent(PLATFORM_DOCK_CHANGED_EVENT))
}

export function resetPlatformDockPaths(tier: PlatformDesktopTier = loadPlatformDesktopTier()) {
  savePlatformDockPaths(defaultDockPathsForTier(tier))
}

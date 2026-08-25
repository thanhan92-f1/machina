// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { Link, useLocation } from 'react-router'
import { usePlatformInfo } from '../contexts/PlatformInfoContext'
import { LayoutGrid, Puzzle, Server, HardDrive, Plus, GitBranch, Shield, Network, Key, Disc, Cpu, Layers, Globe, Camera, Scale, KeyRound, Map } from 'lucide-react'
import { statusActionLinkClasses } from '../utils/semanticColors'

const TABS = [
  { to: '/fleet-cloud', label: 'Overview', icon: LayoutGrid, end: true },
  { to: '/fleet-cloud/instances', label: 'Instances', icon: Server },
  { to: '/fleet-cloud/images', label: 'Images', icon: HardDrive },
  { to: '/fleet-cloud/volumes', label: 'Volumes', icon: Disc },
  { to: '/fleet-cloud/volume-snapshots', label: 'Snapshots', icon: Camera },
  { to: '/fleet-cloud/flavors', label: 'Flavors', icon: Cpu },
  { to: '/fleet-cloud/server-groups', label: 'Groups', icon: Layers },
  { to: '/fleet-cloud/networking', label: 'Network', icon: Network },
  { to: '/fleet-cloud/topology', label: 'Topology', icon: Map },
  { to: '/fleet-cloud/floating-ips', label: 'FIPs', icon: Globe },
  { to: '/fleet-cloud/heat', label: 'Heat', icon: Layers },
  { to: '/fleet-cloud/load-balancers', label: 'LBs', icon: Scale },
  { to: '/fleet-cloud/identity', label: 'Identity', icon: KeyRound },
  { to: '/fleet-cloud/keypairs', label: 'Keys', icon: Key },
  { to: '/fleet-cloud/security-groups', label: 'Security', icon: Shield },
  { to: '/fleet-cloud/create', label: 'Create', icon: Plus },
  { to: '/fleet-cloud/migrations', label: 'Migrations', icon: GitBranch, requiresHypersdk: true },
] as const

export default function FleetCloudSubNav() {
  const { pathname } = useLocation()
  const { info } = usePlatformInfo()
  const hypersdkEnabled = Boolean(info?.hypersdk?.enabled)
  const tabs = TABS.filter((t) => !('requiresHypersdk' in t && t.requiresHypersdk) || hypersdkEnabled)

  return (
    <>
      {Boolean(info?.control_plane?.proxy_url) && (
        <div className="mb-3 flex flex-wrap items-center gap-3 text-xs">
          <Link to="/platform" className={`inline-flex items-center gap-1.5 ${statusActionLinkClasses('info')}`}>
            <LayoutGrid className="w-3.5 h-3.5" />
            Platform desktop
          </Link>
          <Link to="/platform/integrations" className="inline-flex items-center gap-1.5 text-slate-500 hover:text-sky-300">
            <Puzzle className="w-3.5 h-3.5" />
            Integrations
          </Link>
        </div>
      )}
    <nav
      className="mb-6 flex flex-wrap gap-1 p-1 rounded-xl border border-sky-500/25 bg-sky-950/20 backdrop-blur-sm"
      aria-label="Fleet Cloud"
    >
      {tabs.map(({ to, label, icon: Icon, ...rest }) => {
        const end = 'end' in rest && rest.end
        const active = end ? pathname === to : pathname === to || pathname.startsWith(`${to}/`)
        return (
          <Link
            key={to}
            to={to}
            className={`inline-flex items-center gap-2 px-3 py-2 rounded-lg text-sm font-medium transition ${
              active
                ? 'bg-sky-600 text-white shadow-md shadow-sky-600/25'
                : 'text-slate-400 hover:text-slate-100 hover:bg-slate-800/60'
            }`}
          >
            <Icon className="w-4 h-4 shrink-0" />
            {label}
          </Link>
        )
      })}
    </nav>
    </>
  )
}

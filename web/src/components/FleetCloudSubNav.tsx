// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { Link, useLocation } from 'react-router'
import { usePlatformInfo } from '../contexts/PlatformInfoContext'

const TABS = [
  { to: '/fleet-cloud', label: 'Overview', end: true },
  { to: '/fleet-cloud/instances', label: 'Instances' },
  { to: '/fleet-cloud/images', label: 'Images' },
  { to: '/fleet-cloud/volumes', label: 'Volumes' },
  { to: '/fleet-cloud/volume-snapshots', label: 'Snapshots' },
  { to: '/fleet-cloud/flavors', label: 'Flavors' },
  { to: '/fleet-cloud/server-groups', label: 'Groups' },
  { to: '/fleet-cloud/networking', label: 'Network' },
  { to: '/fleet-cloud/topology', label: 'Topology' },
  { to: '/fleet-cloud/floating-ips', label: 'Floating IPs' },
  { to: '/fleet-cloud/heat', label: 'Heat' },
  { to: '/fleet-cloud/load-balancers', label: 'Load balancers' },
  { to: '/fleet-cloud/identity', label: 'Identity' },
  { to: '/fleet-cloud/keypairs', label: 'Keys' },
  { to: '/fleet-cloud/security-groups', label: 'Security' },
  { to: '/fleet-cloud/create', label: 'Create' },
] as const

export default function FleetCloudSubNav() {
  const { pathname } = useLocation()
  const { info } = usePlatformInfo()
  const hypersdkEnabled = Boolean(info?.hypersdk?.enabled)
  const tabs = TABS.filter((t) => !('requiresHypersdk' in t && t.requiresHypersdk) || hypersdkEnabled)

  return (
    <>
      {Boolean(info?.control_plane?.proxy_url) && (
        <div className="mb-6 flex flex-wrap items-center gap-6 text-[15px]">
          <Link to="/platform" className="apple-link">
            Platform
          </Link>
          <Link to="/platform/settings?section=integrations" className="apple-link">
            Integrations
          </Link>
        </div>
      )}
      <nav
        className="mb-12 flex flex-wrap gap-x-6 gap-y-3 border-b border-[var(--apple-hairline)] pb-4"
        aria-label="Fleet Cloud"
      >
        {tabs.map(({ to, label, ...rest }) => {
          const end = 'end' in rest && rest.end
          const active = end ? pathname === to : pathname === to || pathname.startsWith(`${to}/`)
          return (
            <Link
              key={to}
              to={to}
              className={`text-[15px] tracking-tight transition-colors pb-1 border-b-2 -mb-[17px] ${
                active
                  ? 'text-[var(--text-primary)] border-[var(--text-primary)]'
                  : 'text-[var(--text-muted)] border-transparent hover:text-[var(--text-primary)]'
              }`}
            >
              {label}
            </Link>
          )
        })}
      </nav>
    </>
  )
}

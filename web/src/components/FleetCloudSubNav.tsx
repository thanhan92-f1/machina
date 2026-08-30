// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useEffect, useRef, useState } from 'react'
import { Link, useLocation } from 'react-router'
import { ChevronDown } from 'lucide-react'
import { usePlatformInfo } from '../contexts/PlatformInfoContext'

type Tab = { to: string; label: string; end?: boolean }

/** Primary destinations — kept as pills; everything else under More. */
const PRIMARY: Tab[] = [
  { to: '/fleet-cloud', label: 'Overview', end: true },
  { to: '/fleet-cloud/instances', label: 'Instances' },
  { to: '/fleet-cloud/images', label: 'Images' },
  { to: '/fleet-cloud/volumes', label: 'Volumes' },
  { to: '/fleet-cloud/create', label: 'Create' },
]

const MORE: Tab[] = [
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
]

function tabActive(pathname: string, tab: Tab): boolean {
  if (tab.end) return pathname === tab.to
  return pathname === tab.to || pathname.startsWith(`${tab.to}/`)
}

function pillClass(active: boolean): string {
  return `inline-flex items-center rounded-full px-3.5 py-1.5 text-[13px] font-medium tracking-tight transition-colors ${
    active
      ? 'bg-[var(--accent)] text-[var(--text-on-accent,#fff)]'
      : 'text-[var(--text-secondary)] hover:text-[var(--text-primary)] hover:bg-[var(--surface-hover,rgba(16,20,28,0.05))]'
  }`
}

/** Compact Fleet Cloud context pills — replaces the old 15-tab strip. */
export default function FleetCloudSubNav() {
  const { pathname } = useLocation()
  const { info } = usePlatformInfo()
  const [moreOpen, setMoreOpen] = useState(false)
  const moreRef = useRef<HTMLDivElement>(null)
  const moreActive = MORE.some((t) => tabActive(pathname, t))

  useEffect(() => {
    if (!moreOpen) return
    const onDoc = (e: MouseEvent) => {
      if (moreRef.current && !moreRef.current.contains(e.target as Node)) setMoreOpen(false)
    }
    document.addEventListener('mousedown', onDoc)
    return () => document.removeEventListener('mousedown', onDoc)
  }, [moreOpen])

  useEffect(() => {
    setMoreOpen(false)
  }, [pathname])

  return (
    <div className="mb-8 space-y-4">
      {Boolean(info?.control_plane?.proxy_url) && (
        <div className="flex flex-wrap items-center gap-6 text-[15px]">
          <Link to="/platform" className="apple-link">
            Platform
          </Link>
          <Link to="/platform/settings?section=integrations" className="apple-link">
            Integrations
          </Link>
        </div>
      )}
      <nav className="flex flex-wrap items-center gap-1.5" aria-label="Fleet Cloud">
        {PRIMARY.map((tab) => (
          <Link key={tab.to} to={tab.to} className={pillClass(tabActive(pathname, tab))}>
            {tab.label}
          </Link>
        ))}
        <div className="relative" ref={moreRef}>
          <button
            type="button"
            className={`${pillClass(moreActive)} gap-1`}
            aria-expanded={moreOpen}
            aria-haspopup="menu"
            onClick={() => setMoreOpen((v) => !v)}
          >
            More
            <ChevronDown className={`h-3.5 w-3.5 transition-transform ${moreOpen ? 'rotate-180' : ''}`} />
          </button>
          {moreOpen ? (
            <div
              role="menu"
              className="absolute left-0 top-full z-40 mt-1.5 min-w-[12rem] rounded-xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)] py-1 shadow-lg"
            >
              {MORE.map((tab) => {
                const active = tabActive(pathname, tab)
                return (
                  <Link
                    key={tab.to}
                    role="menuitem"
                    to={tab.to}
                    className={`block px-3.5 py-2 text-[13px] transition-colors ${
                      active
                        ? 'bg-[var(--accent-soft,rgba(0,113,227,0.1))] text-[var(--text-primary)] font-medium'
                        : 'text-[var(--text-secondary)] hover:bg-[var(--surface-hover)] hover:text-[var(--text-primary)]'
                    }`}
                  >
                    {tab.label}
                  </Link>
                )
              })}
            </div>
          ) : null}
        </div>
      </nav>
    </div>
  )
}

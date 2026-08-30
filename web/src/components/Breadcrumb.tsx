// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { Link, useLocation } from 'react-router'
import { ChevronRight } from 'lucide-react'
import { routeLabels } from '../utils/routes'
import { useBreadcrumbNameValue } from '../contexts/BreadcrumbNameContext'

const ID_PATTERN = /^[0-9a-f]{8,}(-[0-9a-f]{4,})*$/i

export default function Breadcrumb() {
  const { pathname } = useLocation()
  const entityName = useBreadcrumbNameValue()

  if (pathname === '/') return null

  const segments = pathname.split('/').filter(Boolean)
  const crumbs: { path: string; label: string }[] = []

  let cumulative = ''
  for (let i = 0; i < segments.length; i++) {
    const seg = segments[i]
    cumulative += `/${seg}`
    const isLast = i === segments.length - 1
    const fromRouteLabels = routeLabels[cumulative]
    const label =
      fromRouteLabels ||
      (entityName && isLast && ID_PATTERN.test(seg) ? entityName : decodeURIComponent(seg))
    crumbs.push({ path: cumulative, label })
  }

  if (crumbs.length === 0) return null

  return (
    <nav className="mb-10 flex items-center gap-2 text-[13px] tracking-tight" aria-label="Breadcrumb">
      <Link to="/" className="text-[var(--text-muted)] hover:text-[var(--link)] transition-colors">
        Home
      </Link>
      {crumbs.map((crumb, i) => {
        const isLast = i === crumbs.length - 1
        return (
          <span key={crumb.path} className="flex items-center gap-2 min-w-0">
            <ChevronRight className="w-3 h-3 text-[var(--text-muted)] opacity-50 shrink-0" strokeWidth={1.75} />
            {isLast ? (
              <span className="text-[var(--text-secondary)] truncate">{crumb.label}</span>
            ) : (
              <Link to={crumb.path} className="text-[var(--text-muted)] hover:text-[var(--link)] transition-colors truncate">
                {crumb.label}
              </Link>
            )}
          </span>
        )
      })}
    </nav>
  )
}

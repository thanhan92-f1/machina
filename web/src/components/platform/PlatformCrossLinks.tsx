// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { Link } from 'react-router'
import { ExternalLink } from 'lucide-react'

export function PlatformClassicToolLinks({
  tools,
}: {
  tools: Array<{ title: string; href: string; description?: string }>
}) {
  if (tools.length === 0) return null
  return (
    <ul className="grid gap-2 sm:grid-cols-2">
      {tools.map((t) => (
        <li key={t.href}>
          <Link
            to={t.href}
            className="block rounded-xl border border-white/[0.06] bg-slate-950/30 px-3 py-2.5 hover:border-sky-500/30 transition"
          >
            <span className="text-sm font-medium text-slate-200 flex items-center gap-1.5">
              {t.title}
              <ExternalLink className="w-3 h-3 text-slate-500" />
            </span>
            {t.description && <span className="text-xs text-slate-500 mt-0.5 block leading-relaxed">{t.description}</span>}
          </Link>
        </li>
      ))}
    </ul>
  )
}

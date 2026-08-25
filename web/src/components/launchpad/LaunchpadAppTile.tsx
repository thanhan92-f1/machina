// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useState } from 'react'
import { useNavigate } from 'react-router'
import { Copy, ExternalLink, Pin, Route, Stethoscope } from 'lucide-react'
import { gradientForName, MacGlassPanel } from '../platform/mac/PlatformMacUi'
import type { LaunchpadApp } from '../../api/launchpad'
import {
  copyLaunchpadUrl,
  launchpadDetailPath,
  launchpadStatusLabel,
  launchpadStatusTone,
  openLaunchpadApp,
  pinLaunchpadApp,
} from '../../utils/launchpadHelpers'
import { statusToneClass } from '../../utils/semanticColors'
import { useToastContext } from '../../contexts/ToastContext'

type Props = {
  app: LaunchpadApp
  onInspect?: () => void
  onDiagnose?: () => void
  onOpen?: () => void
}

export default function LaunchpadAppTile({ app, onInspect, onDiagnose, onOpen }: Props) {
  const navigate = useNavigate()
  const toast = useToastContext()
  const [hovered, setHovered] = useState(false)
  const tone = launchpadStatusTone(app.status) as 'ok' | 'warn' | 'error' | 'neutral'
  const subtitle = app.description?.trim() || app.category || 'Infrastructure app'
  const initial = app.displayName.trim().charAt(0).toUpperCase() || '?'
  const broken = app.status === 'broken' || app.status === 'degraded'

  const inspect = () => {
    if (onInspect) {
      onInspect()
      return
    }
    navigate(`${launchpadDetailPath(app)}?inspect=1`)
  }

  const diagnose = () => {
    if (onDiagnose) {
      onDiagnose()
      return
    }
    inspect()
  }

  const open = () => {
    if (onOpen) {
      onOpen()
      return
    }
    void openLaunchpadApp(app)
  }

  return (
    <div
      className="h-full"
      data-testid={`launchpad-tile-${app.slug}`}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
    <MacGlassPanel
      className={`launchpad-app-tile h-full transition-all duration-200 hover:scale-[1.01] hover:border-orange-500/30 hover:shadow-lg hover:shadow-orange-500/10 ${
        broken ? 'border-amber-500/40' : ''
      }`}
    >
      <div className="flex flex-col gap-3 h-full relative">
        <div className="flex items-start justify-between gap-3">
          <div
            className={`w-12 h-12 rounded-2xl bg-gradient-to-br ${gradientForName(app.displayName)} flex items-center justify-center text-white font-semibold shadow-lg shadow-black/20`}
            aria-hidden
          >
            {initial}
          </div>
          <span
            className={`text-[10px] font-semibold uppercase tracking-wide px-2 py-1 rounded-full border border-white/10 ${statusToneClass(tone as 'ok' | 'warn' | 'error' | 'info' | 'neutral')}`}
          >
            {launchpadStatusLabel(app.status)}
          </span>
        </div>
        <div>
          <h3 className="text-base font-semibold text-slate-50">{app.displayName}</h3>
          <p className="text-xs text-slate-400 mt-1 line-clamp-2">{subtitle}</p>
        </div>
        {broken && app.statusMessage ? (
          <p className="text-xs text-amber-300/90 line-clamp-2">{app.statusMessage}</p>
        ) : null}

        {hovered ? (
          <div className="mt-auto flex flex-wrap gap-2 pt-1">
            <button type="button" className="btn-primary text-xs inline-flex items-center gap-1.5" onClick={open}>
              <ExternalLink className="w-3.5 h-3.5" />
              Open
            </button>
            <button
              type="button"
              className="btn-secondary text-xs inline-flex items-center gap-1.5"
              onClick={() => void pinLaunchpadApp(app).then(() => toast.success('Pinned to dock'))}
            >
              <Pin className="w-3.5 h-3.5" />
              Pin
            </button>
            <button
              type="button"
              className="btn-secondary text-xs inline-flex items-center gap-1.5"
              onClick={() => void copyLaunchpadUrl(app).then(() => toast.success('URL copied'))}
            >
              <Copy className="w-3.5 h-3.5" />
              Copy URL
            </button>
            <button type="button" className="btn-secondary text-xs inline-flex items-center gap-1.5" onClick={inspect}>
              <Route className="w-3.5 h-3.5" />
              Inspect
            </button>
          </div>
        ) : (
          <div className="mt-auto flex flex-wrap gap-2 pt-1">
            {broken ? (
              <button type="button" className="btn-primary text-xs inline-flex items-center gap-1.5" onClick={diagnose}>
                <Stethoscope className="w-3.5 h-3.5" />
                Diagnose
              </button>
            ) : (
              <button type="button" className="btn-primary text-xs inline-flex items-center gap-1.5" onClick={open}>
                <ExternalLink className="w-3.5 h-3.5" />
                Open
              </button>
            )}
          </div>
        )}
      </div>
    </MacGlassPanel>
    </div>
  )
}

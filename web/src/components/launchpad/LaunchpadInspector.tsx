// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useEffect, useState } from 'react'
import { ExternalLink, Loader2 } from 'lucide-react'
import { Link } from 'react-router'
import ErrorBanner from '../ErrorBanner'
import { MacSheet } from '../platform/mac/PlatformMacUi'
import type { LaunchpadApp, LaunchpadDiagnosis } from '../../api/launchpad'
import { getLaunchpadDiagnosis } from '../../api/launchpad'
import { formatUserError } from '../../utils/apiError'
import LaunchpadRouteLens from './LaunchpadRouteLens'

type Props = {
  app: LaunchpadApp | null
  open: boolean
  onClose: () => void
}

export default function LaunchpadInspector({ app, open, onClose }: Props) {
  const [diagnosis, setDiagnosis] = useState<LaunchpadDiagnosis | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!open || !app) {
      setDiagnosis(null)
      setError(null)
      return
    }
    let cancelled = false
    setLoading(true)
    setError(null)
    void getLaunchpadDiagnosis(app.id)
      .then((d) => {
        if (!cancelled) setDiagnosis(d)
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(formatUserError(e))
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [app, open])

  if (!app) return null

  return (
    <MacSheet
      open={open}
      onClose={onClose}
      title={app.displayName}
      subtitle="Diagnose route and backend health"
      wide
    >
      <div className="space-y-5" data-testid="launchpad-inspector">
        {error ? <ErrorBanner message={error} /> : null}
        {loading ? (
          <div className="flex items-center gap-2 text-sm text-slate-400 py-6">
            <Loader2 className="w-4 h-4 animate-spin" />
            Loading diagnosis…
          </div>
        ) : null}
        {diagnosis ? (
          <>
            {(diagnosis.problem || diagnosis.cause) && (
              <div className="rounded-xl border border-amber-500/30 bg-amber-500/10 p-4 space-y-2">
                {diagnosis.problem ? (
                  <p className="text-sm font-medium text-amber-200">{diagnosis.problem}</p>
                ) : null}
                {diagnosis.cause ? <p className="text-xs text-amber-100/80">{diagnosis.cause}</p> : null}
              </div>
            )}
            <LaunchpadRouteLens diagnosis={diagnosis} />
            {diagnosis.suggestedActions?.length ? (
              <div className="space-y-2">
                <h4 className="text-xs font-semibold uppercase tracking-wide text-slate-400">Suggested actions</h4>
                <div className="flex flex-wrap gap-2">
                  {diagnosis.suggestedActions.map((action) => (
                    <Link
                      key={action.href}
                      to={action.href}
                      className="btn-secondary text-xs inline-flex items-center gap-1.5"
                      onClick={onClose}
                    >
                      <ExternalLink className="w-3.5 h-3.5" />
                      {action.label}
                    </Link>
                  ))}
                </div>
              </div>
            ) : null}
          </>
        ) : null}
      </div>
    </MacSheet>
  )
}

// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useMemo, useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router'
import { ArrowLeft, LayoutGrid, Loader2 } from 'lucide-react'
import ErrorBanner from '../../components/ErrorBanner'
import LaunchpadAppTile from '../../components/launchpad/LaunchpadAppTile'
import LaunchpadInspector from '../../components/launchpad/LaunchpadInspector'
import OperatingSurfaceLayout from '../../components/platform/OperatingSurfaceLayout'
import PlatformPageChrome from '../../components/platform/PlatformPageChrome'
import { listLaunchpadCatalog, type LaunchpadApp } from '../../api/launchpad'
import { formatUserError } from '../../utils/apiError'
import { groupAppsBySpace, launchpadSpaceById } from '../../utils/launchpadSpaces'
import { launchpadDetailPath } from '../../utils/launchpadHelpers'
import { useBreadcrumbName } from '../../contexts/BreadcrumbNameContext'

export default function PlatformLaunchpadSpace() {
  const { spaceId = '' } = useParams()
  const navigate = useNavigate()
  const space = launchpadSpaceById(spaceId)
  useBreadcrumbName(space?.label ?? '')
  const [apps, setApps] = useState<LaunchpadApp[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [inspectApp, setInspectApp] = useState<LaunchpadApp | null>(null)

  const load = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      const catalog = await listLaunchpadCatalog()
      setApps(catalog.filter((a) => a.visibility?.published !== false))
    } catch (e: unknown) {
      setError(formatUserError(e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void load()
  }, [load])

  const spaceApps = useMemo(() => {
    if (!space) return []
    return groupAppsBySpace(apps).get(space.id) ?? []
  }, [apps, space])

  return (
    <PlatformPageChrome
      title={space?.label ?? 'Space'}
      subtitle={space?.description ?? 'Launchpad space'}
      icon={<LayoutGrid className="w-6 h-6 text-orange-400" />}
      contentClassName="space-y-4"
    >
      <OperatingSurfaceLayout testId={`platform-launchpad-space-${spaceId}`}>
        <Link to="/platform/launchpad" className="inline-flex items-center gap-1.5 text-xs text-slate-400 hover:text-orange-300">
          <ArrowLeft className="w-3.5 h-3.5" />
          Back to Launchpad
        </Link>

        {error ? <ErrorBanner message={error} onRetry={() => void load()} /> : null}

        {loading ? (
          <div className="flex items-center gap-2 text-slate-400 text-sm py-8">
            <Loader2 className="w-4 h-4 animate-spin" />
            Loading apps…
          </div>
        ) : !space ? (
          <p className="text-sm text-slate-400">Unknown space.</p>
        ) : (
          <section className="space-y-4">
            <p className="text-sm text-slate-400">
              {spaceApps.length} app{spaceApps.length === 1 ? '' : 's'} in {space.label}
            </p>
            <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
              {spaceApps.map((app) => (
                <LaunchpadAppTile
                  key={app.id}
                  app={app}
                  onOpen={() => navigate(launchpadDetailPath(app))}
                  onInspect={() => setInspectApp(app)}
                  onDiagnose={() => setInspectApp(app)}
                />
              ))}
            </div>
          </section>
        )}
      </OperatingSurfaceLayout>

      <LaunchpadInspector app={inspectApp} open={!!inspectApp} onClose={() => setInspectApp(null)} />
    </PlatformPageChrome>
  )
}

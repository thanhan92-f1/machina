// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { useNavigate } from 'react-router'
import { Lightbulb, Workflow } from 'lucide-react'
import OperatingSurfaceLayout from '../../components/platform/OperatingSurfaceLayout'
import PlatformEmptyState from '../../components/platform/PlatformEmptyState'
import PlatformPageChrome, { PlatformBackLink, PlatformRefreshButton } from '../../components/platform/PlatformPageChrome'
import RemediateChips from '../../components/platform/RemediateChips'
import { createVmBackup, listPlatformRecommendations, setVmHa, type PlatformRecommendation } from '../../api/platform'
import { useToastContext } from '../../contexts/ToastContext'
import { formatUserError } from '../../utils/apiError'

export default function PlatformRecommendations() {
  const toast = useToastContext()
  const navigate = useNavigate()
  const [rows, setRows] = useState<PlatformRecommendation[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  const load = useCallback(async () => {
    setError(null)
    setLoading(true)
    try {
      setRows(await listPlatformRecommendations())
    } catch (e: unknown) {
      setError(formatUserError(e))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => { void load() }, [load])

  const execute = async (r: PlatformRecommendation) => {
    try {
      const vmIds = (r.object_ref?.vm_ids as string[] | undefined) ?? []
      if (r.fix_action === 'bulk_backup') {
        await Promise.all(vmIds.slice(0, 5).map((id) => createVmBackup(id)))
        toast.success('Backup tasks queued')
      } else if (r.fix_action === 'bulk_ha') {
        await Promise.all(vmIds.slice(0, 5).map((id) => setVmHa(id, { enabled: true })))
        toast.success('HA enabled on selected VMs')
      } else if (r.fix_action === 'open_hosts') {
        navigate('/platform/hosts')
      } else if (r.fix_action === 'open_vms') {
        navigate('/platform/vms')
      }
      await load()
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    }
  }

  return (
    <PlatformPageChrome
      eyebrow="Platform"
      error={error}
      onErrorRetry={() => void load()}
      loading={loading && rows.length === 0 && !error}
      prepend={<PlatformBackLink to="/platform/operations" label="Operations" />}
      title="Recommendations"
      subtitle="Live analysis from your cluster — not static placeholders."
      icon={<Lightbulb className="w-6 h-6 text-[var(--text-muted)]" />}
      actions={<PlatformRefreshButton onClick={() => void load()} />}
      className="w-full max-w-none"
      contentClassName="space-y-4"
    >
      <OperatingSurfaceLayout testId="platform-recommendations-page">
      <RemediateChips />
      <ul className="space-y-4">
        {rows.map((r) => (
          <li key={r.id} className="card p-5 space-y-2">
            <span className="text-[10px] font-bold uppercase tracking-wider text-[var(--text-muted)]">{r.impact}</span>
            <h3 className="font-semibold text-lg">{r.title}</h3>
            <p className="text-sm text-[var(--text-muted)]"><strong className="text-[var(--text-secondary)]">Why?</strong> {r.why}</p>
            <p className="text-xs text-[var(--text-muted)]">Risk: {r.risk}</p>
            <div className="flex gap-2 pt-2">
              <button type="button" className="btn-primary text-sm" onClick={() => void execute(r)}>{r.action}</button>
            </div>
          </li>
        ))}
        {rows.length === 0 && !error && (
          <PlatformEmptyState title="No recommendations" subtitle="Your estate looks good — check back after changes to hosts or VMs." />
        )}
      </ul>
      </OperatingSurfaceLayout>
    </PlatformPageChrome>
  )
}

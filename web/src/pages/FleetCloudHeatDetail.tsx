// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useRef, useState } from 'react'
import { usePlatformTabState } from '../hooks/usePlatformTabState'
import { Link, useNavigate, useParams } from 'react-router'
import { ArrowLeft, Layers, Trash2 } from 'lucide-react'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import PageSkeleton from '../components/PageSkeleton'
import { deleteStack, getStack, type NativeStack } from '../api/stacks'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { useBreadcrumbName } from '../contexts/BreadcrumbNameContext'

// resources/template only — native stacks have no Heat-style events log, no
// live outputs, and (unlike Heat) no in-place template update: recreate instead.
const HEAT_TABS = ['overview', 'resources', 'template'] as const
type Tab = (typeof HEAT_TABS)[number]

// Native stacks — not gated by <OpenStackGate> (see OpenStackHeat.tsx).
export default function OpenStackHeatDetailPage() {
  return <OpenStackHeatDetailContent />
}

function OpenStackHeatDetailContent() {
  const { id } = useParams<{ name: string; id: string }>()
  const toast = useToastContext()
  const navigate = useNavigate()
  const [stack, setStack] = useState<NativeStack | null>(null)
  const [tab, setTab] = usePlatformTabState(HEAT_TABS, { defaultTab: 'overview' })
  const [loading, setLoading] = useState(true)
  useBreadcrumbName(stack?.name)
  const loadStackSeq = useRef(0)

  const loadStack = useCallback(async () => {
    if (!id) return
    // Last-response-wins: only the newest load may commit so a stale fetch for a
    // prior stack can't overwrite the one now shown.
    const seq = ++loadStackSeq.current
    const alive = () => seq === loadStackSeq.current
    setLoading(true)
    try {
      const s = await getStack(id)
      if (!alive()) return
      setStack(s)
    } catch (e: unknown) {
      if (!alive()) return
      toast.error(formatUserError(e))
      setStack(null)
    } finally {
      if (alive()) setLoading(false)
    }
  }, [id, toast])

  useEffect(() => { void loadStack() }, [loadStack])

  if (loading) return <PageSkeleton />
  if (!stack) {
    return (
      <div className="space-y-4">
        <FleetCloudSubNav />
        <Link to="/fleet-cloud/heat" className="text-sky-400 hover:underline">Back</Link>
      </div>
    )
  }

  const tabs: { id: Tab; label: string }[] = [
    { id: 'overview', label: 'Overview' },
    { id: 'resources', label: 'Resources' },
    { id: 'template', label: 'Template' },
  ]

  return (
    <PageLayout
      hideHeader
      className="max-w-4xl"
      prepend={<><FleetCloudSubNav /></>}
    >
      <Link to="/fleet-cloud/heat" className="inline-flex items-center gap-2 text-slate-400 hover:text-slate-200 text-sm">
        <ArrowLeft className="w-4 h-4" /> Stacks
      </Link>
      <h1 className="text-2xl font-semibold flex items-center gap-2">
        <Layers className="w-7 h-7 text-violet-400" /> {stack.name}
      </h1>

      <div className="flex flex-wrap gap-2 border-b border-slate-700 pb-2">
        {tabs.map((t) => (
          <button
            key={t.id}
            type="button"
            className={`px-3 py-1.5 rounded-lg text-sm ${tab === t.id ? 'bg-violet-600/30 text-violet-200' : 'text-slate-400 hover:text-slate-200'}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === 'overview' && (
        <dl className="grid sm:grid-cols-2 gap-4 rounded-xl border border-slate-700 p-4 text-sm">
          <div><dt className="text-xs text-slate-500 uppercase">ID</dt><dd className="font-mono mt-1 break-all">{stack.id}</dd></div>
          <div><dt className="text-xs text-slate-500 uppercase">Status</dt><dd className="mt-1">{stack.status}</dd></div>
          <div className="sm:col-span-2"><dt className="text-xs text-slate-500 uppercase">Last error</dt><dd className="mt-1 text-slate-400">{stack.last_error || '—'}</dd></div>
        </dl>
      )}

      {tab === 'resources' ? (
        <div className="overflow-x-auto rounded-xl border border-slate-700">
          <table className="w-full text-sm" aria-label="Stack resources">
            <thead className="bg-slate-900/80 text-slate-400 text-left">
              <tr>
                <th scope="col" className="px-3 py-2">Name</th>
                <th scope="col" className="px-3 py-2">Kind</th>
                <th scope="col" className="px-3 py-2">ID</th>
              </tr>
            </thead>
            <tbody>
              {stack.resources_json.map((r) => (
                <tr key={r.id} className="border-t border-slate-800">
                  <td className="px-3 py-2">{r.name}</td>
                  <td className="px-3 py-2 text-slate-400">{r.kind}</td>
                  <td className="px-3 py-2 font-mono text-xs">{r.id}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {stack.resources_json.length === 0 && <p className="p-4 text-slate-500 text-sm">No resources created yet.</p>}
        </div>
      ) : tab === 'template' ? (
        <pre className="w-full font-mono text-xs px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 overflow-x-auto">
          {JSON.stringify(stack.template_json, null, 2)}
        </pre>
      ) : null}

      <button type="button" className="px-3 py-1.5 rounded-lg border border-red-600/50 text-red-300 text-sm inline-flex items-center gap-1"
        onClick={async () => {
          if (!confirm(`Delete stack ${stack.name}?`)) return
          try {
            await deleteStack(stack.id)
            toast.success('Deleted')
            navigate('/fleet-cloud/heat')
          } catch (e: unknown) { toast.error(formatUserError(e)) }
        }}>
        <Trash2 className="w-4 h-4" /> Delete stack
      </button>
      <FleetCloudFooter />
    </PageLayout>
  )
}

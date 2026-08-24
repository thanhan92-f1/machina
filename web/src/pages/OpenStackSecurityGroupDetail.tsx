// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useParams } from 'react-router'
import { ArrowLeft, Loader2, Shield } from 'lucide-react'
import {
  createSecurityGroupRule,
  deleteSecurityGroup,
  deleteSecurityGroupRule,
  getSecurityGroup,
  listSecurityGroupRules,
  type NativeSecurityGroup,
  type NativeSecurityGroupRule,
} from '../api/securityGroups'
import OpenStackSubNav from '../components/OpenStackSubNav'
import OpenStackFooter from '../components/OpenStackFooter'
import PageLayout from '../components/PageLayout'
import PageSkeleton from '../components/PageSkeleton'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { statusActionLinkClasses } from '../utils/semanticColors'
import { useBreadcrumbName } from '../contexts/BreadcrumbNameContext'

// Native security groups — not gated by <OpenStackGate> (see OpenStackSecurityGroups.tsx).
export default function OpenStackSecurityGroupDetailPage() {
  return <OpenStackSecurityGroupDetailContent />
}

function OpenStackSecurityGroupDetailContent() {
  const { id } = useParams<{ id: string }>()
  const toast = useToastContext()
  const [group, setGroup] = useState<NativeSecurityGroup | null>(null)
  const [rules, setRules] = useState<NativeSecurityGroupRule[]>([])
  const [loading, setLoading] = useState(true)
  useBreadcrumbName(group?.name)
  const loadSeq = useRef(0)

  const load = useCallback(async () => {
    if (!id) return
    // Last-response-wins: only the newest load may commit so a stale fetch for a
    // prior security group can't overwrite the one now shown.
    const seq = ++loadSeq.current
    const alive = () => seq === loadSeq.current
    setLoading(true)
    try {
      const [g, r] = await Promise.all([getSecurityGroup(id), listSecurityGroupRules(id)])
      if (!alive()) return
      setGroup(g)
      setRules(r)
    } catch (e: unknown) {
      if (!alive()) return
      toast.error(formatUserError(e))
      setGroup(null)
    } finally {
      if (alive()) setLoading(false)
    }
  }, [id, toast])

  useEffect(() => { void load() }, [load])

  if (loading) return <PageSkeleton />
  if (!group) {
    return (
      <div className="space-y-4">
        <OpenStackSubNav />
        <Link to="/openstack/security-groups" className="text-sky-400 hover:underline">Back</Link>
      </div>
    )
  }

  return (
    <PageLayout
      hideHeader
      className="max-w-3xl"
      prepend={<><OpenStackSubNav /></>}
    >
      <Link to="/openstack/security-groups" className="inline-flex items-center gap-2 text-slate-400 hover:text-slate-200 text-sm">
        <ArrowLeft className="w-4 h-4" /> Security groups
      </Link>
      <h1 className="text-2xl font-semibold flex items-center gap-2">
        <Shield className="w-7 h-7 text-sky-400" />
        {group.name}
      </h1>
      {group.description && <p className="text-sm text-slate-400">{group.description}</p>}
      <p className="text-xs text-amber-400/90">
        Advisory only — rule enforcement isn't wired to the firewall yet.
      </p>
      <dl className="grid sm:grid-cols-2 gap-4 rounded-xl border border-slate-700 p-4 text-sm">
        <div><dt className="text-xs text-slate-500 uppercase">ID</dt><dd className="font-mono text-slate-200 mt-1 break-all">{group.id}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Rules</dt><dd className="text-slate-200 mt-1">{rules.length}</dd></div>
      </dl>
      <div className="flex flex-wrap gap-2">
        <button type="button" className="px-3 py-1.5 rounded-lg border border-slate-600 text-sm"
          onClick={async () => {
            try {
              await createSecurityGroupRule(group.id, {
                direction: 'ingress',
                protocol: 'tcp',
                port_min: 22,
                port_max: 22,
                remote_cidr: '0.0.0.0/0',
              })
              toast.success('Added SSH rule')
              void load()
            } catch (e: unknown) { toast.error(formatUserError(e)) }
          }}>Add SSH ingress</button>
        <button type="button" className="px-3 py-1.5 rounded-lg border border-red-500/50 text-red-300 text-sm"
          onClick={async () => {
            if (!confirm(`Delete security group ${group.name}?`)) return
            try {
              await deleteSecurityGroup(group.id)
              toast.success('Deleted')
              window.location.href = '/openstack/security-groups'
            } catch (e: unknown) { toast.error(formatUserError(e)) }
          }}>Delete group</button>
      </div>
      <section className="rounded-xl border border-slate-700 p-4">
        <h2 className="text-sm font-medium text-slate-300 mb-3">Rules</h2>
        {rules.length === 0 ? (
          <p className="text-sm text-slate-500">No rules.</p>
        ) : (
          <ul className="space-y-2 text-xs font-mono">
            {rules.map((r) => (
              <li key={r.id} className="flex flex-wrap items-center gap-2 rounded-lg border border-slate-800 px-3 py-2 text-slate-300">
                <span>{r.direction}</span>
                <span>{r.protocol || 'any'}</span>
                {(r.port_min != null || r.port_max != null) && (
                  <span>{r.port_min ?? '—'}–{r.port_max ?? '—'}</span>
                )}
                {r.remote_cidr && <span>{r.remote_cidr}</span>}
                <button type="button" className={statusActionLinkClasses('error', 'ml-auto')}
                  onClick={async () => {
                    try {
                      await deleteSecurityGroupRule(r.id)
                      toast.success('Rule deleted')
                      void load()
                    } catch (e: unknown) { toast.error(formatUserError(e)) }
                  }}>Delete</button>
              </li>
            ))}
          </ul>
        )}
      </section>
      <OpenStackFooter />
    </PageLayout>
  )
}

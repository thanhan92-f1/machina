// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router'
import { KeyRound, Loader2, Plus } from 'lucide-react'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import EmptyState from '../components/EmptyState'
import { createProject, listProjectRegistry, type NativeProject } from '../api/nativeProjects'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { statusToneClass } from '../utils/semanticColors'

// Native project registry — this feature is SQLite-native and does not depend
// on a wired external cloud (there's no old external-cloud gate
// component to gate it behind either: the daemon's external-cloud-client
// integration has since been fully removed). Unlike Keystone, there is no
// per-project *user* catalog here — identity/login is Machina's own
// PAM/OIDC/LDAP/SAML auth; project membership references an existing Machina
// user (see the project detail page), so the standalone "create a Keystone
// user with a password" flow has no native equivalent.
export default function FleetCloudIdentityPage() {
  return <FleetCloudIdentityContent />
}

function FleetCloudIdentityContent() {
  const toast = useToastContext()
  const [projects, setProjects] = useState<NativeProject[]>([])
  const [loading, setLoading] = useState(true)
  const [projectName, setProjectName] = useState('')

  const load = useCallback(async () => {
    setLoading(true)
    try {
      const p = await listProjectRegistry()
      setProjects(p)
    } catch (e: unknown) {
      toast.error(formatUserError(e))
      setProjects([])
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => { void load() }, [load])

  return (
    <PageLayout
      hideHeader
      prepend={<><FleetCloudSubNav /></>}
      ><p className="apple-eyebrow">Fleet Cloud</p>
      <h1 className="page-title flex items-center gap-3">
        <KeyRound className={`w-7 h-7 ${statusToneClass('warn')}`} /> Projects
      </h1>
      <p className="text-[var(--text-muted)] text-sm">
        Project registry with membership/roles. Login identity is Machina's own PAM/OIDC/LDAP/SAML
        auth (see Settings) — add existing Machina users to a project from its detail page.
      </p>

      <div className="rounded-2xl border border-[var(--apple-hairline)] bg-[var(--apple-surface)] p-4 flex flex-wrap gap-2 items-center">
        <input aria-label="New project name" value={projectName} onChange={(e) => setProjectName(e.target.value)} placeholder="New project name"
          className="input-field text-sm" />
        <button type="button" className="btn-secondary text-sm inline-flex items-center gap-1"
          onClick={async () => {
            if (!projectName.trim()) return
            try {
              await createProject({ name: projectName.trim() })
              toast.success('Project created')
              setProjectName('')
              void load()
            } catch (e: unknown) { toast.error(formatUserError(e)) }
          }}>
          <Plus className="w-4 h-4" /> Create project
        </button>
      </div>

      {loading ? (
        <Loader2 className="w-8 h-8 animate-spin text-[var(--accent)] mx-auto" />
      ) : projects.length === 0 ? (
        <EmptyState title="No projects" description="No projects in the registry yet." />
      ) : (
        <div className="overflow-x-auto apple-surface rounded-2xl">
          <table className="apple-table" aria-label="Projects">
            <thead>
              <tr><th scope="col" className="px-3 py-2">Name</th><th scope="col" className="px-3 py-2">ID</th><th scope="col" className="px-3 py-2">Enabled</th></tr>
            </thead>
            <tbody>
              {projects.map((p) => (
                <tr key={p.id} className="border-t border-[var(--apple-hairline)]">
                  <td className="px-3 py-2">
                    <Link to={`/fleet-cloud/identity/projects/${p.id}`} className="text-[var(--accent)] hover:underline">{p.name}</Link>
                  </td>
                  <td className="px-3 py-2 font-mono text-xs">{p.id}</td>
                  <td className="px-3 py-2">{p.enabled ? 'yes' : 'no'}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      <FleetCloudFooter />
    </PageLayout>
  )
}

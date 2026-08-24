// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router'
import { createTemplate, deleteTemplate, listTemplates, type NativeTemplate } from '../api/nativeTemplates'
import { useToastContext } from '../contexts/ToastContext'
import ConfirmDialog from '../components/ConfirmDialog'
import FleetCloudFooter from '../components/FleetCloudFooter'
import { Cloud, RefreshCw, Plus, Trash2 } from 'lucide-react'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import PageLayout from '../components/PageLayout'
import { formatUserError } from '../utils/apiError'
import { statusToneClass } from '../utils/semanticColors'

// Native golden-image catalog — not gated by <OpenStackGate>. See
// api/nativeTemplates.ts: unlike Glance there's no byte-upload here (register an
// existing on-host disk path, or publish one from a running VM).
export default function OpenStackImagesPage() {
  return <OpenStackImagesContent />
}

function OpenStackImagesContent() {
  const [images, setImages] = useState<NativeTemplate[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<NativeTemplate | null>(null)
  const [deleting, setDeleting] = useState(false)
  const [name, setName] = useState('')
  const [version, setVersion] = useState('1.0')
  const [sourceDisk, setSourceDisk] = useState('')
  const [creating, setCreating] = useState(false)
  const toast = useToastContext()

  const load = useCallback(async () => {
    try {
      setLoadError(null)
      const list = await listTemplates()
      setImages(list)
    } catch (e: unknown) {
      const msg = formatUserError(e)
      setLoadError(msg)
      toast.error(`Failed to load images: ${msg}`)
    } finally {
      setLoading(false)
    }
  }, [toast])

  useEffect(() => { void load() }, [load])

  const handleDelete = async () => {
    if (!deleteTarget) return
    setDeleting(true)
    try {
      await deleteTemplate(deleteTarget.id)
      toast.success(`Deleted image '${deleteTarget.name}'`)
      setDeleteTarget(null)
      await load()
    } catch (e: unknown) {
      toast.error(formatUserError(e))
    } finally {
      setDeleting(false)
    }
  }

  return (
    <PageLayout
      prepend={<><FleetCloudSubNav /></>}
      title="Images"
      subtitle="Golden-image catalog for instance creation."
      icon={<Cloud className="w-7 h-7 text-sky-400" />}
      error={loadError}
      errorTitle="Failed to load images"
      technicalDetail={loadError}
      errorTone="red"
      onErrorRetry={() => void load()}
      onErrorDismiss={() => setLoadError(null)}
      actions={
        <div className="flex gap-2 flex-wrap">
          <Link
            to="/disk-images"
            className="inline-flex items-center gap-2 px-3 py-2 rounded-lg border border-sky-500/40 text-sky-300 hover:bg-sky-500/10 text-sm"
          >
            Manage disk images
          </Link>
          <Link
            to="/fleet-cloud/create"
            className="inline-flex items-center gap-2 px-4 py-2 rounded-lg bg-sky-600 hover:bg-sky-500 text-white text-sm"
          >
            <Plus className="w-4 h-4" />
            Boot instance
          </Link>
          <button
            type="button"
            onClick={() => { setLoading(true); void load() }}
            className="inline-flex items-center gap-2 px-3 py-2 rounded-lg border border-slate-600 text-slate-200 hover:bg-slate-800 text-sm"
          >
            <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
            Refresh
          </button>
        </div>
      }
    >
      <div className="rounded-xl border border-slate-700 p-4 space-y-3">
        <h2 className="text-sm font-medium text-slate-300 flex items-center gap-2"><Plus className="w-4 h-4" /> Register image</h2>
        <div className="grid sm:grid-cols-3 gap-3">
          <input aria-label="Image name" value={name} onChange={(e) => setName(e.target.value)} placeholder="Name"
            className="px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-sm" />
          <input aria-label="Version" value={version} onChange={(e) => setVersion(e.target.value)} placeholder="Version"
            className="px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-sm" />
          <input aria-label="Source disk path" value={sourceDisk} onChange={(e) => setSourceDisk(e.target.value)} placeholder="/path/to/golden.qcow2"
            className="px-3 py-2 rounded-lg bg-slate-900 border border-slate-700 text-sm font-mono" />
        </div>
        <button type="button" disabled={creating || !name.trim() || !sourceDisk.trim()}
          className="px-3 py-1.5 rounded-lg bg-sky-600 hover:bg-sky-500 text-white text-sm disabled:opacity-40"
          onClick={async () => {
            setCreating(true)
            try {
              await createTemplate({ name: name.trim(), version: version.trim() || '1.0', source_disk: sourceDisk.trim() })
              toast.success('Image registered')
              setName(''); setSourceDisk('')
              void load()
            } catch (e: unknown) { toast.error(formatUserError(e)) } finally { setCreating(false) }
          }}>{creating ? 'Registering…' : 'Register'}</button>
      </div>

      <div className="overflow-x-auto rounded-xl border border-slate-700/80">
        <table className="w-full text-sm" aria-label="Images">
          <thead className="bg-slate-900/80 text-slate-400 text-left">
            <tr>
              <th scope="col" className="px-4 py-3">Name</th>
              <th scope="col" className="px-4 py-3">Version</th>
              <th scope="col" className="px-4 py-3">Status</th>
              <th scope="col" className="px-4 py-3">Source</th>
              <th scope="col" className="px-4 py-3 w-16" />
            </tr>
          </thead>
          <tbody className="divide-y divide-slate-800">
            {loading && images.length === 0 && (
              <tr><td colSpan={5} className="px-4 py-8 text-center text-slate-500">Loading…</td></tr>
            )}
            {!loading && images.length === 0 && (
              <tr><td colSpan={5} className="px-4 py-8 text-center text-slate-500">No images found.</td></tr>
            )}
            {images.map((img) => (
              <tr key={img.id} className="hover:bg-slate-800/40">
                <td className="px-4 py-3">
                  <Link to={`/fleet-cloud/images/${img.id}`} className="text-slate-200 hover:text-sky-300 hover:underline">
                    {img.name}
                  </Link>
                  <div className="text-xs text-slate-500 font-mono truncate max-w-xs">{img.id}</div>
                </td>
                <td className="px-4 py-3 text-slate-300">{img.version}</td>
                <td className="px-4 py-3 text-slate-300">{img.approval_status}</td>
                <td className="px-4 py-3 text-slate-500 font-mono text-xs truncate max-w-xs">{img.source_disk}</td>
                <td className="px-4 py-3">
                  <button
                    type="button"
                    title="Delete image"
                    onClick={() => setDeleteTarget(img)}
                    className={`p-2 rounded hover:bg-[color-mix(in_srgb,var(--machina-status-error)_25%,transparent)] ${statusToneClass('error')}`}
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      <FleetCloudFooter />

      <ConfirmDialog
        open={!!deleteTarget}
        title="Delete image"
        message={`Permanently delete ${deleteTarget?.name} from the image catalog?`}
        confirmLabel={deleting ? 'Deleting…' : 'Delete'}
        variant="danger"
        onConfirm={handleDelete}
        onCancel={() => setDeleteTarget(null)}
      />
    </PageLayout>
  )
}

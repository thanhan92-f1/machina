// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useRef, useState } from 'react'
import { Link, useParams } from 'react-router'
import { ArrowLeft, HardDrive } from 'lucide-react'
import { getTemplate, type NativeTemplate } from '../api/nativeTemplates'
import FleetCloudSubNav from '../components/FleetCloudSubNav'
import FleetCloudFooter from '../components/FleetCloudFooter'
import PageLayout from '../components/PageLayout'
import PageSkeleton from '../components/PageSkeleton'
import { useToastContext } from '../contexts/ToastContext'
import { formatUserError } from '../utils/apiError'
import { useBreadcrumbName } from '../contexts/BreadcrumbNameContext'

// Native golden-image catalog — there's no old external-cloud gate component to
// gate it behind (the daemon's external-cloud-client integration has since
// been fully removed); see FleetCloudImages.tsx.
export default function FleetCloudImageDetailPage() {
  return <FleetCloudImageDetailContent />
}

function FleetCloudImageDetailContent() {
  const { id } = useParams<{ id: string }>()
  const toast = useToastContext()
  const [image, setImage] = useState<NativeTemplate | null>(null)
  const [loading, setLoading] = useState(true)
  useBreadcrumbName(image?.name)
  const loadSeq = useRef(0)

  const load = useCallback(async () => {
    if (!id) return
    // Last-response-wins: only the newest load may commit so a stale fetch for a
    // prior image can't overwrite the one now shown.
    const seq = ++loadSeq.current
    const alive = () => seq === loadSeq.current
    setLoading(true)
    try {
      const img = await getTemplate(id)
      if (!alive()) return
      setImage(img)
    } catch (e: unknown) {
      if (!alive()) return
      toast.error(formatUserError(e))
      setImage(null)
    } finally {
      if (alive()) setLoading(false)
    }
  }, [id, toast])

  useEffect(() => {
    void load()
  }, [load])

  if (loading) {
    return <PageSkeleton />
  }

  if (!image) {
    return (
      <div className="space-y-4">
        <FleetCloudSubNav />
        <p className="text-slate-400">Image not found.</p>
        <Link to="/fleet-cloud/images" className="text-sky-400 hover:underline">Back to images</Link>
      </div>
    )
  }

  return (
    <PageLayout
      hideHeader
      className="max-w-3xl"
      prepend={<><FleetCloudSubNav /></>}
    >
      <Link to="/fleet-cloud/images" className="inline-flex items-center gap-2 text-slate-400 hover:text-slate-200 text-sm">
        <ArrowLeft className="w-4 h-4" /> Images
      </Link>
      <h1 className="text-2xl font-semibold flex items-center gap-2">
        <HardDrive className="w-7 h-7 text-sky-400" />
        {image.name}
      </h1>
      <dl className="grid sm:grid-cols-2 gap-4 rounded-xl border border-slate-700 p-4 text-sm">
        <div><dt className="text-xs text-slate-500 uppercase">ID</dt><dd className="font-mono text-slate-200 mt-1 break-all">{image.id}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Status</dt><dd className="text-slate-200 mt-1">{image.approval_status}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Version</dt><dd className="text-slate-200 mt-1">{image.version}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">OS family</dt><dd className="text-slate-200 mt-1">{image.os_family || '—'}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Category</dt><dd className="text-slate-200 mt-1">{image.category}</dd></div>
        <div><dt className="text-xs text-slate-500 uppercase">Cloud-init</dt><dd className="text-slate-200 mt-1">{image.cloud_init ? 'yes' : 'no'}</dd></div>
        <div className="sm:col-span-2"><dt className="text-xs text-slate-500 uppercase">Source disk</dt><dd className="font-mono text-slate-200 mt-1 break-all">{image.source_disk}</dd></div>
        {image.description && (
          <div className="sm:col-span-2"><dt className="text-xs text-slate-500 uppercase">Description</dt><dd className="text-slate-200 mt-1">{image.description}</dd></div>
        )}
      </dl>
      <FleetCloudFooter />
    </PageLayout>
  )
}

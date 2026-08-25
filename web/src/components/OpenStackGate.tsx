// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { ReactNode } from 'react'
import { usePlatformInfo } from '../contexts/PlatformInfoContext'
import { useOpenStackConnection } from '../hooks/useOpenStackConnection'
import OpenStackSetupPanel from './OpenStackSetupPanel'
import OpenStackSubNav from './OpenStackSubNav'
import OpenStackStatusBar from './OpenStackStatusBar'
import OpenStackUnreachablePanel from './OpenStackUnreachablePanel'

/** Renders children when OpenStack API is live; otherwise setup or unreachable panels. */
export default function OpenStackGate({
  children,
  title,
}: {
  children: ReactNode
  title?: string
}) {
  const { loading: platformLoading } = usePlatformInfo()
  const { phase, loading: connLoading, configured } = useOpenStackConnection()

  if (platformLoading || (configured && connLoading)) {
    return (
      <div className="flex items-center justify-center h-40">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2 border-sky-500" />
      </div>
    )
  }

  if (phase === 'off' || phase === 'needsWire') {
    return (
      <div className="space-y-6 animate-fade-in">
        {title && <h1 className="text-2xl font-semibold text-slate-100">{title}</h1>}
        <OpenStackSubNav />
        <OpenStackStatusBar />
        <OpenStackSetupPanel />
      </div>
    )
  }

  if (phase === 'unreachable') {
    return (
      <div className="space-y-6 animate-fade-in">
        {title && <h1 className="text-2xl font-semibold text-slate-100">{title}</h1>}
        <OpenStackSubNav />
        <OpenStackStatusBar />
        <OpenStackUnreachablePanel />
      </div>
    )
  }

  return <>{children}</>
}

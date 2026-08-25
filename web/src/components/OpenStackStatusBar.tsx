// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { Link } from 'react-router'
import { Settings, Wifi, WifiOff, AlertCircle, RefreshCw } from 'lucide-react'
import { useOpenStackConnection } from '../hooks/useOpenStackConnection'
import { usePlatformInfo } from '../contexts/PlatformInfoContext'
import { useToastContext } from '../contexts/ToastContext'
import { useState } from 'react'
import { formatUserError, sanitizeErrorText } from '../utils/apiError'
import { statusActionLinkClasses, statusChipClasses, statusSurfaceClasses, statusToneClass } from '../utils/semanticColors'

/** Live OpenStack connection summary for cloud pages. */
export default function OpenStackStatusBar() {
  const { info } = usePlatformInfo()
  const toast = useToastContext()
  const { phase, status, testConnection, cloudName, computeLive, glanceLive, connectionHint } =
    useOpenStackConnection()
  const [testing, setTesting] = useState(false)

  if (phase === 'off' || phase === 'needsWire') {
    return (
      <div className={`mb-4 flex flex-wrap items-center gap-3 rounded-xl border px-4 py-3 text-sm ${statusSurfaceClasses('warn')}`}>
        <span className={`inline-flex items-center gap-2 font-medium ${statusToneClass('warn')}`}>
          <AlertCircle className={`w-4 h-4 ${statusToneClass('warn')}`} />
          OpenStack not wired on this host
        </span>
        <span className="text-xs text-slate-400">
          enabled={info?.openstack?.enabled ? 'yes' : 'no'} · configured=
          {info?.openstack?.configured ? 'yes' : 'no'}
        </span>
        <Link
          to="/settings?openstack=1"
          className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg border text-xs ml-auto ${statusActionLinkClasses('warn')}`}
        >
          <Settings className="w-3.5 h-3.5" />
          Wire &amp; settings
        </Link>
      </div>
    )
  }

  const reachable = phase === 'live'

  return (
    <div
      className={`mb-4 flex flex-wrap items-center gap-3 rounded-xl px-4 py-3 text-sm border ${statusSurfaceClasses(reachable ? 'ok' : 'error')}`}
    >
      <span className="inline-flex items-center gap-2 font-medium text-slate-200">
        {reachable ? (
          <Wifi className={`w-4 h-4 ${statusToneClass('ok')}`} />
        ) : (
          <WifiOff className={`w-4 h-4 ${statusToneClass('error')}`} />
        )}
        {cloudName || 'OpenStack'}
        {!reachable && <span className={`text-xs font-normal ${statusToneClass('error')}`}>· unreachable</span>}
      </span>
      {reachable && (
        <span className="text-xs text-slate-400">
          Keystone
          {computeLive ? ' · Nova' : ' · Nova off'}
          {glanceLive ? ' · Glance' : ' · Glance off'}
          {status?.neutron_reachable ? ' · Neutron' : status?.neutron_reachable === false ? ' · Neutron off' : ''}
          {status?.cinder_reachable ? ' · Cinder' : status?.cinder_reachable === false ? ' · Cinder off' : ''}
        </span>
      )}
      {reachable && !computeLive && connectionHint && (
        <span className={`text-xs max-w-lg ${statusToneClass('warn')} opacity-90`} title={connectionHint}>
          {connectionHint}
        </span>
      )}
      {reachable && computeLive && status?.instance_count != null && (
        <span className="text-slate-400">
          {status.instance_count} instance{status.instance_count === 1 ? '' : 's'}
        </span>
      )}
      {reachable && glanceLive && status?.image_count != null && (
        <span className="text-slate-400">
          · {status.image_count} image{status.image_count === 1 ? '' : 's'}
        </span>
      )}
      {info?.openstack?.upload_enabled && reachable && glanceLive && (
        <span className={statusChipClasses('ok')}>
          Glance upload on
        </span>
      )}
      {status?.error && reachable && (
        <span className={`text-xs truncate max-w-md opacity-90 ${statusToneClass('warn')}`} title={status.error}>
          {sanitizeErrorText(status.error)}
        </span>
      )}
      {status?.error && !reachable && (
        <span className={`text-xs truncate max-w-md ${statusToneClass('error')}`} title={status.error}>
          {sanitizeErrorText(status.error)}
        </span>
      )}
      <div className="flex flex-wrap gap-2 ml-auto">
        <button
          type="button"
          disabled={testing}
          onClick={async () => {
            setTesting(true)
            try {
              const s = await testConnection()
              toast.success(s.reachable ? 'OpenStack OK' : 'Still unreachable')
            } catch (e: unknown) {
              toast.error(formatUserError(e))
            } finally {
              setTesting(false)
            }
          }}
          className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg border text-xs disabled:opacity-50 ${statusActionLinkClasses(reachable ? 'info' : 'error')}`}
        >
          <RefreshCw className={`w-3.5 h-3.5 ${testing ? 'animate-spin' : ''}`} />
          {testing ? 'Testing…' : 'Test'}
        </button>
        <Link
          to="/settings?openstack=1"
          className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg border border-slate-600 text-slate-300 hover:bg-slate-800 text-xs"
        >
          <Settings className="w-3.5 h-3.5" />
          Settings
        </Link>
      </div>
    </div>
  )
}

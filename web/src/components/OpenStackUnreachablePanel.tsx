// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { Link } from 'react-router'
import { Cloud, Settings, RefreshCw } from 'lucide-react'
import { useState } from 'react'
import { useOpenStackConnection } from '../hooks/useOpenStackConnection'
import { useToastContext } from '../contexts/ToastContext'
import ErrorBanner from './ErrorBanner'
import CopyButton from './CopyButton'
import { VERIFY_COMMANDS, WIRE_SCRIPT, openStackErrorHints } from '../utils/openstackHints'
import { formatUserError, sanitizeErrorText } from '../utils/apiError'
import { statusSurfaceClasses, statusToneClass } from '../utils/semanticColors'

/** Shown when OpenStack is configured in Machina but Keystone/API is not reachable. */
export default function OpenStackUnreachablePanel() {
  const { status, testConnection, cloudName } = useOpenStackConnection()
  const toast = useToastContext()
  const [testing, setTesting] = useState(false)
  const rawError = status?.error || 'Could not reach OpenStack API'
  const error = sanitizeErrorText(rawError)
  const hints = openStackErrorHints(rawError)

  return (
    <div className="space-y-4">
      <div className={`rounded-xl border p-5 ${statusSurfaceClasses('error')}`}>
        <div className="flex items-start gap-3">
          <div className={`w-10 h-10 rounded-xl flex items-center justify-center shrink-0 ${statusSurfaceClasses('error')}`}>
            <Cloud className={`w-5 h-5 ${statusToneClass('error')}`} />
          </div>
          <div className="min-w-0 flex-1 space-y-2">
            <h2 className={`text-lg font-semibold ${statusToneClass('error')}`}>
              Cloud configured but not reachable
              {cloudName ? ` (${cloudName})` : ''}
            </h2>
            <p className="text-sm text-slate-400">
              Machina has credentials on this host, but Nova/Glance APIs are not responding. Install or start
              OpenStack services before managing instances from the UI.
            </p>
          </div>
        </div>
      </div>

      <ErrorBanner
        title="Connection error"
        headline={error.length > 200 ? `${error.slice(0, 197)}…` : error}
        hints={hints}
        technicalDetail={rawError}
        tone="red"
        actions={
          <>
            <button
              type="button"
              disabled={testing}
              onClick={async () => {
                setTesting(true)
                try {
                  const s = await testConnection()
                  toast.success(s.reachable ? 'OpenStack is reachable' : 'Still unreachable — see error')
                } catch (e: unknown) {
                  toast.error(formatUserError(e))
                } finally {
                  setTesting(false)
                }
              }}
              className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg border border-sky-500/40 text-sky-200 hover:bg-sky-500/10 text-xs disabled:opacity-50"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${testing ? 'animate-spin' : ''}`} />
              Test connection
            </button>
            <CopyButton text={VERIFY_COMMANDS} label="Copy verify commands" />
            <Link
              to="/settings?openstack=1"
              className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg border border-slate-600 text-slate-300 hover:bg-slate-800 text-xs"
            >
              <Settings className="w-3.5 h-3.5" />
              Settings
            </Link>
          </>
        }
      />

      <div className="rounded-lg bg-slate-950/60 border border-slate-700/60 p-3 text-xs text-slate-400 font-mono">
        <p className="text-slate-500 mb-2">Re-wire after fixing keystonerc:</p>
        <code className={`block whitespace-pre-wrap break-all ${statusToneClass('warn')} opacity-90`}>{WIRE_SCRIPT}</code>
        <div className="mt-2">
          <CopyButton text={WIRE_SCRIPT} label="Copy wire script" />
        </div>
      </div>
    </div>
  )
}

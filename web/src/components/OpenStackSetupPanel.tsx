// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { Link } from 'react-router'
import { Cloud, Terminal, Settings, AlertCircle } from 'lucide-react'
import { usePlatformInfo } from '../contexts/PlatformInfoContext'
import { isOpenStackNavEnabled } from '../utils/routes'
import { statusSurfaceClasses, statusToneClass } from '../utils/semanticColors'

/** Shown when OpenStack routes are visited but the daemon is not wired. */
export default function OpenStackSetupPanel({ compact = false }: { compact?: boolean }) {
  const { info } = usePlatformInfo()
  const ready = isOpenStackNavEnabled(info?.openstack)
  if (ready) return null

  const enabled = Boolean(info?.openstack?.enabled)
  const configured = Boolean(info?.openstack?.configured)

  return (
    <div
      className={`rounded-xl border ${statusSurfaceClasses('warn', compact ? 'p-4' : 'p-6')}`}
    >
      <div className="flex items-start gap-3">
        <div className={`w-10 h-10 rounded-xl flex items-center justify-center shrink-0 ${statusSurfaceClasses('warn')}`}>
          <Cloud className={`w-5 h-5 ${statusToneClass('warn')}`} />
        </div>
        <div className="min-w-0 flex-1 space-y-3">
          <div>
            <h2 className={`font-semibold ${statusToneClass('warn')} ${compact ? 'text-base' : 'text-lg'}`}>
              Wire OpenStack on this host
            </h2>
            <p className="text-sm text-slate-400 mt-1">
              {enabled && !configured
                ? 'OpenStack is enabled in machina config but missing cloud_name, auth_url, or clouds.yaml.'
                : 'Nova & Glance management appears in the menu after the daemon can reach Keystone.'}
            </p>
            <p className="text-xs text-slate-500 mt-2 font-mono">
              enabled={enabled ? 'yes' : 'no'} · configured={configured ? 'yes' : 'no'}
              {info?.openstack?.cloud_name ? ` · cloud=${info.openstack.cloud_name}` : ''}
            </p>
          </div>
          <div className="rounded-lg bg-slate-950/60 border border-slate-700/60 p-3 text-xs text-slate-400 font-mono leading-relaxed">
            <div className="flex items-center gap-2 text-slate-500 mb-2">
              <Terminal className="w-3.5 h-3.5" />
              Packstack / RDO on this hypervisor
            </div>
            <code className={`block whitespace-pre-wrap break-all ${statusToneClass('warn')} opacity-90`}>
              sudo /usr/local/share/machina/scripts/openstack-wire-cloud.sh /root/keystonerc_admin packstack{'\n'}
              sudo systemctl restart machina-daemon
            </code>
          </div>
          <div className="flex flex-wrap gap-2">
            <Link
              to="/settings?openstack=1"
              className="inline-flex items-center gap-2 px-3 py-2 rounded-lg bg-amber-600 hover:bg-amber-500 text-white text-sm font-medium"
            >
              <Settings className="w-4 h-4" />
              OpenStack settings
            </Link>
            <a
              href="/api/v1/openstack/status"
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-2 px-3 py-2 rounded-lg border border-slate-600 text-slate-300 hover:bg-slate-800 text-sm"
            >
              API status
            </a>
          </div>
          {!compact && (
            <p className="text-xs text-slate-500 flex items-start gap-2">
              <AlertCircle className="w-3.5 h-3.5 shrink-0 mt-0.5" />
              Credentials stay on the host (clouds.yaml, openrc, or OS_*). Never stored in the browser.
            </p>
          )}
        </div>
      </div>
    </div>
  )
}

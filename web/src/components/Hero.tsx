// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { type ReactNode } from 'react'
import { Link } from 'react-router'
import { Activity, Lock, Shield, Boxes, KeyRound, Wifi, WifiOff, Cloud } from 'lucide-react'
import { usePlatformInfo } from '../contexts/PlatformInfoContext'
import { useOpenStackConnection } from '../hooks/useOpenStackConnection'
import { statusBadgeClasses, statusBorderClass } from '../utils/semanticColors'

interface HeroProps {
  /** Page title (rendered as gradient headline). */
  title: string
  /** Single-line subtitle below the title. */
  subtitle?: string
  /** Optional icon shown next to the title. */
  icon?: ReactNode
  /** Right-side controls (primary actions, filters, etc.). */
  actions?: ReactNode
  /** Inline children rendered under the badge row (filters, search, etc.). */
  children?: ReactNode
  /** Hide the default capability/runtime badges (e.g. for compact pages). */
  hideBadges?: boolean
}

interface BadgeProps {
  on: boolean
  label: string
  icon?: ReactNode
  /** Tooltip with the resolved value (e.g. "namespace=default"). */
  title?: string
  tone?: 'default' | 'info' | 'warn' | 'error'
}

function Badge({ on, label, icon, title, tone = 'default' }: BadgeProps) {
  const onCls =
    tone === 'info'
      ? `${statusBadgeClasses('info')} border ${statusBorderClass('info')}`
      : tone === 'warn'
        ? `${statusBadgeClasses('warn')} border ${statusBorderClass('warn')}`
        : tone === 'error'
          ? `${statusBadgeClasses('error')} border ${statusBorderClass('error')}`
          : `${statusBadgeClasses('ok')} border ${statusBorderClass('ok')}`
  const offCls = 'border-slate-700/60 bg-slate-800/40 text-slate-400'
  return (
    <span
      title={title}
      className={`inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium ${on ? onCls : offCls}`}
    >
      {icon}
      <span>{label}</span>
    </span>
  )
}

/**
 * Aether-style page hero: gradient title, capability badges from
 * `/system/platform-info` + `/auth/providers`, and a live indicator driven by
 * the daemon SSE bus. Use at the top of any page that wants the shell look.
 */
export default function Hero({ title, subtitle, icon, actions, children, hideBadges }: HeroProps) {
  const { info, providers, liveConnected, loading } = usePlatformInfo()
  const { phase: osPhase, cloudName: osCloud, status: openstackStatus } = useOpenStackConnection()

  const osBadge = (() => {
    if (osPhase === 'live') {
      const partial = openstackStatus && !openstackStatus.compute_reachable
      return {
        on: true,
        label: partial ? `OpenStack: Keystone` : `OpenStack: ${osCloud || 'live'}`,
        tone: partial ? ('warn' as const) : ('info' as const),
        title: partial
          ? `Identity OK; Nova/Glance may be down — ${openstackStatus?.error || 'see OpenStack overview'}`
          : `Cloud ${osCloud}; upload=${info?.openstack?.upload_enabled ? 'on' : 'off'}`,
        to: '/openstack',
      }
    }
    if (osPhase === 'unreachable') {
      return {
        on: true,
        label: 'OpenStack: unreachable',
        tone: 'error' as const,
        title: 'Configured but Keystone/API not reachable',
        to: '/openstack',
      }
    }
    if (osPhase === 'needsWire') {
      return {
        on: false,
        label: 'OpenStack: not wired',
        tone: 'warn' as const,
        title: 'Enable [openstack] and run openstack-wire-cloud.sh',
        to: '/settings?openstack=1',
      }
    }
    return {
      on: false,
      label: 'OpenStack off',
      tone: 'warn' as const,
      title: 'Enable [openstack] in /etc/machina/config.toml',
      to: '/settings?openstack=1',
    }
  })()

  return (
    <div className="mb-6 rounded-2xl border border-slate-700/40 bg-gradient-to-br from-slate-900/70 via-slate-900/40 to-slate-800/40 p-5 backdrop-blur">
      <div className="flex flex-wrap items-start justify-between gap-4">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-3">
            {icon && <span className="text-sky-300">{icon}</span>}
            <h1 className="bg-gradient-to-r from-white via-sky-100 to-violet-200 bg-clip-text text-2xl font-semibold tracking-tight text-transparent">
              {title}
            </h1>
          </div>
          {subtitle && <p className="mt-1 text-sm text-slate-400">{subtitle}</p>}
        </div>
        {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
      </div>

      {!hideBadges && (
        <div className="mt-4 flex flex-wrap items-center gap-2">
          {info?.host?.os_pretty_name ? (
            <Badge
              on
              label={info.host.os_pretty_name}
              title="Hypervisor host OS (from /etc/os-release)"
              tone="info"
            />
          ) : !loading ? (
            <Badge
              on={false}
              label="Host OS unknown"
              title="platform-info did not report os_pretty_name"
              tone="warn"
            />
          ) : null}
          <Badge
            on={liveConnected}
            label={liveConnected ? 'Live' : 'Reconnecting…'}
            icon={liveConnected ? <Wifi className="h-3 w-3" /> : <WifiOff className="h-3 w-3" />}
            title="Live updates from /api/v1/events/stream"
            tone={liveConnected ? 'default' : 'warn'}
          />
          <Badge
            on={!loading && Boolean(info?.tls.enabled)}
            label={info?.tls.enabled ? 'TLS' : 'No TLS'}
            icon={<Lock className="h-3 w-3" />}
            title={info?.tls.enabled ? 'HTTPS terminated by daemon' : 'Daemon serves HTTP only'}
            tone={info?.tls.enabled ? 'default' : 'warn'}
          />
          <Badge
            on={Boolean(providers?.pam.enabled)}
            label={`PAM (${info?.auth.pam_service ?? 'sshd'})`}
            icon={<Shield className="h-3 w-3" />}
            title="PAM stack used by /auth/login"
            tone="info"
          />
          <Badge
            on={Boolean(providers?.oidc.enabled)}
            label={providers?.oidc.enabled ? 'OIDC' : 'OIDC off'}
            icon={<KeyRound className="h-3 w-3" />}
            title={providers?.oidc.button_label}
            tone="info"
          />
          <Link to={osBadge.to} className="inline-flex no-underline" title={osBadge.title}>
            <Badge
              on={osBadge.on}
              label={osBadge.label}
              icon={<Cloud className="h-3 w-3" />}
              tone={osBadge.tone}
            />
          </Link>
          <Badge
            on={Boolean(info?.kubevirt.exec_enabled)}
            label={info?.kubevirt.exec_enabled ? 'KubeVirt: exec' : 'KubeVirt: bundle-only'}
            icon={<Boxes className="h-3 w-3" />}
            title={
              info?.kubevirt.exec_enabled
                ? `kubectl/virtctl on namespace=${info.kubevirt.default_namespace}`
                : 'Daemon will not run kubectl/virtctl; download YAML and apply manually'
            }
            tone={info?.kubevirt.exec_enabled ? 'default' : 'warn'}
          />
          {info?.version && (
            <Badge
              on
              label={`v${info.version}`}
              icon={<Activity className="h-3 w-3" />}
              title="machina-daemon version"
              tone="info"
            />
          )}
        </div>
      )}

      {children && <div className="mt-4">{children}</div>}
    </div>
  )
}

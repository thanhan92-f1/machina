// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { Link } from 'react-router'
import { hubLinkClasses } from '../../utils/semanticColors'
import { Puzzle, Sparkles, Boxes, Server } from 'lucide-react'
import { AppleDestinationList } from '../../components/platform/apple/AppleStoryKit'
import HostEnrollWizard from '../../components/platform/HostEnrollWizard'
import PlatformEmptyState from '../../components/platform/PlatformEmptyState'
import { listPlatformHosts } from '../../api/platform'
import { LaunchpadAppIcon, MacGlassPanel } from '../../components/platform/mac/PlatformMacUi'
import PlatformPageChrome, { PlatformBackLink, platformStatSubtitle } from '../../components/platform/PlatformPageChrome'
import { usePlatformInfo } from '../../contexts/PlatformInfoContext'
import { integrationCards } from '../../utils/platformIntegrationsNav'
import { CLASSIC_TOOL_CARDS, LIBVIRT_ADMIN_TOOL_CARDS } from '../../utils/platformClassicTools'
import { PlatformClassicToolLinks } from '../../components/platform/PlatformCrossLinks'
import PlatformDesktopTierPicker from '../../components/platform/PlatformDesktopTierPicker'
import PlatformIntegrationEmbeds from '../../components/platform/PlatformIntegrationEmbeds'
import { usePlatformDesktopTier } from '../../hooks/usePlatformDesktopTier'

export default function PlatformIntegrations({ embedded }: { embedded?: boolean } = {}) {
  const { info } = usePlatformInfo()
  const [tier, setTier] = usePlatformDesktopTier()
  const [hostCount, setHostCount] = useState<number | null>(null)
  const [enrollOpen, setEnrollOpen] = useState(false)
  const cards = integrationCards(info)
  const enabledCount = cards.filter((c) => c.enabled).length

  const loadHosts = useCallback(async () => {
    try {
      const hosts = await listPlatformHosts()
      setHostCount(hosts.length)
    } catch {
      setHostCount(null)
    }
  }, [])

  useEffect(() => {
    void loadHosts()
  }, [loadHosts])

  return (
    <PlatformPageChrome
      eyebrow="Platform"
      hideHeader={embedded}
      compact={embedded}
      className={embedded ? '' : 'max-w-4xl'}
      prepend={embedded ? undefined : <PlatformBackLink to="/platform" label="Platform" />}
      title={embedded ? undefined : 'Apps & Integrations'}
      subtitle={
        embedded ? undefined : (
          <span className="flex flex-col gap-1">
            <span className="text-[var(--text-muted)]">Fleet Cloud, K8s, migration tools, and classic UI</span>
            {platformStatSubtitle([
              { label: 'Available', value: String(cards.length) },
              { label: 'Enabled', value: String(enabledCount) },
              { label: 'Desktop tier', value: tier.charAt(0).toUpperCase() + tier.slice(1) },
            ])}
          </span>
        )
      }
      icon={embedded ? undefined : <Puzzle className="w-6 h-6 text-[var(--text-muted)]" />}
      contentClassName="space-y-6"
    >
        {hostCount === 0 && (
          <PlatformEmptyState
            icon={Server}
            title="No hypervisors enrolled"
            subtitle="Enroll a host before connecting Fleet Cloud, migration tools, or fleet apps."
          >
            <button type="button" className="tahoe-btn-primary text-sm" onClick={() => setEnrollOpen(true)}>
              Enroll host
            </button>
          </PlatformEmptyState>
        )}

        <AppleDestinationList
          items={cards.map((c) => ({
            to: c.href,
            title: c.title,
            subtitle: [
              c.description,
              !c.enabled ? 'off' : null,
              c.enabled && c.configured === false ? 'needs setup' : null,
            ]
              .filter(Boolean)
              .join(' · '),
          }))}
        />

        <MacGlassPanel title="Fleet apps" subtitle="Launchpad and connected platforms">
          <div className="platform-launchpad-grid grid grid-cols-3 sm:grid-cols-4 md:grid-cols-5 gap-x-4 gap-y-8 -mt-1">
            <Link to="/platform/applications" className="block">
              <LaunchpadAppIcon name="Applications" icon={<Boxes className="w-8 h-8" strokeWidth={1.75} />} />
            </Link>
          </div>
        </MacGlassPanel>

        <MacGlassPanel title="Desktop density">
          <p className="text-sm text-[var(--text-muted)] mb-3">
            Start with <strong className="text-[var(--text-primary)]">Normal</strong> for a clean Finder-style desktop. Switch to Power or Advanced when you need Zeus, firewall modules, and the full sidebar.
          </p>
          <PlatformDesktopTierPicker tier={tier} onChange={setTier} />
        </MacGlassPanel>

        <PlatformIntegrationEmbeds />

        <MacGlassPanel title="Libvirt admin (classic)" subtitle="NW filters, secrets vault, and capability matrix — daemon-only routes">
          <p className="text-sm text-[var(--text-muted)] mb-4 leading-relaxed">
            These tools manage libvirt objects on the co-located hypervisor daemon. They open in the classic Machina shell with the same session.
          </p>
          <PlatformClassicToolLinks tools={LIBVIRT_ADMIN_TOOL_CARDS} />
        </MacGlassPanel>

        <MacGlassPanel title="Classic Machina tools">
          <p className="text-sm text-[var(--text-muted)] mb-4 leading-relaxed">
            Import wizards, libvirt node tools, NW filters, secrets, and the classic audit viewer — same daemon, classic UI chrome.
          </p>
          <PlatformClassicToolLinks tools={CLASSIC_TOOL_CARDS} />
          <p className="text-sm text-[var(--text-muted)] mt-4 pt-4 border-t border-white/[0.06]">
            Host REST catalog:{' '}
            <Link to="/api-docs" className={`hover:underline ${hubLinkClasses()}`}>Classic API explorer</Link>
            {' · '}
            <Link to="/platform/developer" className={`hover:underline ${hubLinkClasses()}`}>Platform Developer console</Link>
          </p>
        </MacGlassPanel>

        <MacGlassPanel title="Leaving the desktop">
          <p className="text-sm text-[var(--text-muted)] leading-relaxed">
            Fleet Cloud, HyperSDK, GuestKit, and classic routes open outside the Platform shell. You stay signed in to the same Machina session — use the sidebar or <Link to="/platform" className={hubLinkClasses()}>Platform home</Link> to return.
          </p>
        </MacGlassPanel>

        <MacGlassPanel title="Need more?">
          <p className="text-sm text-[var(--text-muted)] flex items-center gap-2">
            <Sparkles className="w-4 h-4 text-[var(--accent)]" />
            Switch to <Link to="/platform/settings?section=general" className={hubLinkClasses()}>Settings → Appearance → Advanced</Link> for the full fleet sidebar, Zeus Firewall panes, and developer SDK routes.
          </p>
        </MacGlassPanel>

      <HostEnrollWizard open={enrollOpen} onClose={() => { setEnrollOpen(false); void loadHosts() }} />
    </PlatformPageChrome>
  )
}

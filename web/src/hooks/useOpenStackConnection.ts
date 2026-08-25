// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { useCallback, useEffect, useState } from 'react'
import { getOpenStackStatus, postOpenStackTestConnection, type OpenStackConnectionStatus } from '../api/openstack'
import { usePlatformInfo } from '../contexts/PlatformInfoContext'
import { isOpenStackConfigured } from '../utils/routes'

export type OpenStackPhase = 'off' | 'needsWire' | 'unreachable' | 'live'

export function openStackPhaseFrom(
  enabled: boolean,
  configured: boolean,
  status: OpenStackConnectionStatus | null,
): OpenStackPhase {
  if (!enabled) return 'off'
  if (!configured) return 'needsWire'
  const identityUp = status?.keystone_reachable ?? status?.reachable
  if (!identityUp) return 'unreachable'
  return 'live'
}

/** Nova/compute APIs available (instances, flavors, etc.). */
export function isOpenStackComputeLive(status: OpenStackConnectionStatus | null | undefined): boolean {
  return Boolean(status?.compute_reachable)
}

/** Glance image APIs available. */
export function isOpenStackGlanceLive(status: OpenStackConnectionStatus | null | undefined): boolean {
  return Boolean(status?.glance_reachable)
}

export function useOpenStackConnection() {
  const { info, refreshKey } = usePlatformInfo()
  const os = info?.openstack
  const configured = isOpenStackConfigured(os)
  const [status, setStatus] = useState<OpenStackConnectionStatus | null>(null)
  const [loading, setLoading] = useState(configured)

  const refresh = useCallback(async () => {
    if (!configured) {
      setStatus(null)
      setLoading(false)
      return
    }
    setLoading(true)
    try {
      setStatus(await getOpenStackStatus())
    } catch {
      setStatus(null)
    } finally {
      setLoading(false)
    }
  }, [configured])

  const testConnection = useCallback(async () => {
    const s = await postOpenStackTestConnection()
    setStatus(s)
    return s
  }, [])

  useEffect(() => {
    void refresh()
  }, [refresh, refreshKey])

  const phase = openStackPhaseFrom(Boolean(os?.enabled), Boolean(os?.configured), status)

  const connectionHint =
    phase === 'needsWire'
      ? 'Wire clouds.yaml and restart machina-daemon (Settings → OpenStack).'
      : phase === 'unreachable'
        ? status?.error || 'Keystone unreachable — check auth_url and firewall to port 5000.'
        : phase === 'live' && !isOpenStackComputeLive(status)
          ? 'Keystone OK; Nova compute API is down or nova-compute uses fake.FakeDriver (no real guests).'
          : null

  return {
    phase,
    configured,
    status,
    loading,
    refresh,
    testConnection,
    cloudName: status?.cloud_name || os?.cloud_name || '',
    computeLive: isOpenStackComputeLive(status),
    glanceLive: isOpenStackGlanceLive(status),
    connectionHint,
  }
}

// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { useCallback, useEffect, useState } from 'react'
import { getK8sOverview, type K8sOverview } from '../api/k8s'
import {
  listOpenStackImages,
  listOpenStackInstances,
  listOpenStackNetworks,
  type OpenStackInstance,
} from '../api/openstack'
import { useOpenStackConnection } from './useOpenStackConnection'

export type OpenStackPreviewStats = {
  instances: number
  networks: number
  images: number
  preview: OpenStackInstance[]
}

export function useIntegrationPreviewStats(k8sEnabled: boolean) {
  const openstack = useOpenStackConnection()
  const [osStats, setOsStats] = useState<OpenStackPreviewStats | null>(null)
  const [k8sStats, setK8sStats] = useState<K8sOverview | null>(null)
  const [osLoading, setOsLoading] = useState(false)
  const [k8sLoading, setK8sLoading] = useState(false)
  const [osError, setOsError] = useState<string | null>(null)
  const [k8sError, setK8sError] = useState<string | null>(null)

  const refreshOpenStack = useCallback(async () => {
    if (openstack.phase !== 'live' || !openstack.computeLive) {
      setOsStats(null)
      setOsError(null)
      return
    }
    setOsLoading(true)
    setOsError(null)
    try {
      const [inst, nets, imgs] = await Promise.all([
        listOpenStackInstances({ limit: 5 }),
        listOpenStackNetworks(),
        listOpenStackImages(),
      ])
      setOsStats({
        instances: inst.total ?? inst.instances.length,
        networks: nets.networks.length,
        images: imgs.images.length,
        preview: inst.instances,
      })
    } catch (e: unknown) {
      setOsStats(null)
      setOsError(e instanceof Error ? e.message : 'OpenStack preview failed')
    } finally {
      setOsLoading(false)
    }
  }, [openstack.phase, openstack.computeLive])

  const refreshK8s = useCallback(async () => {
    if (!k8sEnabled) {
      setK8sStats(null)
      setK8sError(null)
      return
    }
    setK8sLoading(true)
    setK8sError(null)
    try {
      setK8sStats(await getK8sOverview())
    } catch (e: unknown) {
      setK8sStats(null)
      setK8sError(e instanceof Error ? e.message : 'Cluster preview failed')
    } finally {
      setK8sLoading(false)
    }
  }, [k8sEnabled])

  useEffect(() => {
    void refreshOpenStack()
  }, [refreshOpenStack, openstack.status])

  useEffect(() => {
    void refreshK8s()
  }, [refreshK8s])

  return {
    openstack,
    osStats,
    k8sStats,
    osLoading,
    k8sLoading,
    osError,
    k8sError,
    refreshOpenStack,
    refreshK8s,
  }
}

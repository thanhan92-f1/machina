// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { readJsonObject } from './client'

const API = '/api/v1'

export interface IntegrationsStatus {
  openstack: {
    configured: boolean
    enabled: boolean
    cloud_name: string
    connected: boolean
    reachable?: boolean
    error?: string | null
    status_url: string
  }
  kubevirt: {
    exec_enabled: boolean
    default_namespace: string
    default_storage_class: string
    routes: Record<string, string>
  }
  automation: {
    worker_interval_secs: number
    last_tick_unix: number | null
    alert_rules_total: number
    alert_rules_enabled: number
    alerts_unacknowledged: number
    routes: Record<string, string>
  }
  k8s: {
    kubeconfig_auto_selected: string | null
    kubectl_args_prefix: string[]
    inventory_history_enabled: boolean
    routes: Record<string, string>
  }
  run_as_user: {
    enabled: boolean
    mode: string
    impersonation_active: boolean
    status_url: string
  }
}

export const getIntegrationsStatus = () =>
  readJsonObject<IntegrationsStatus>(`${API}/integrations/status`)

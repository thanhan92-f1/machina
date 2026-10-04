// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { apiGet, apiPost } from './client'

/** Result of injecting the GuestKit agent into a powered-off VM's disk (daemon, same host as the VM). */
export type AgentInjectReport = {
  vm: string
  disk: string
  binary: string
  dry_run: boolean
  output: string
}

/** True when the daemon you are talking to manages this VM (offline injection runs on that host). */
export const daemonManagesVm = (name: string): Promise<boolean> =>
  apiGet<unknown>(`/api/v1/vms/${encodeURIComponent(name)}`).then(() => true, () => false)

/**
 * True when this daemon knows the inject route (a GET on a POST-only route answers 405; an older daemon answers 404).
 * Checked before anything is shut down so an old daemon never leaves a VM powered off.
 */
export const injectSupported = async (name: string): Promise<boolean> => {
  try {
    const res = await fetch(`/api/v1/vms/${encodeURIComponent(name)}/guest-agent/inject`, { credentials: 'same-origin' })
    return res.status === 405
  } catch {
    return false
  }
}

export const injectGuestAgent = (name: string, dryRun = false) =>
  apiPost<AgentInjectReport>(`/api/v1/vms/${encodeURIComponent(name)}/guest-agent/inject`, { dry_run: dryRun })

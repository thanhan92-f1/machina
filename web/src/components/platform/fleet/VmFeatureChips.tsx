// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { Bot, Cloud, Cpu, Lock, Radio, Server, ShieldCheck, Terminal, Video } from 'lucide-react'
import type { ConsoleHubPlan, PlatformVm } from '../../../api/platform'
import { statusPillClasses } from '../../../utils/semanticColors'

type Tone = 'ok' | 'info' | 'warn' | 'neutral'
type Chip = { key: string; label: string; tone: Tone; icon: typeof Cpu; title?: string }

/** Feature chips derived only from fields the API already returns. */
export function vmFeatures(vm: PlatformVm, plan: ConsoleHubPlan | null): Chip[] {
  const chips: Chip[] = []
  const tools = (vm.guest_tools_status ?? '').toLowerCase()
  if (tools) {
    const ok = tools.includes('ok') || tools.includes('run') || tools.includes('active') || tools.includes('ready')
    chips.push({ key: 'qga', label: ok ? 'Guest agent' : `Guest agent: ${vm.guest_tools_status}`, tone: ok ? 'ok' : 'warn', icon: Bot })
  }
  if (vm.ha_enabled) chips.push({ key: 'ha', label: 'High availability', tone: 'info', icon: ShieldCheck })
  if (plan?.ssh_user && vm.guest_ip) chips.push({ key: 'ssh', label: `SSH · ${plan.ssh_user}`, tone: 'ok', icon: Terminal })
  if (plan?.webrtc_spice_available) chips.push({ key: 'spice', label: 'WebRTC SPICE', tone: 'info', icon: Radio })
  if (plan?.session_recording_enabled) chips.push({ key: 'rec', label: 'Session recording', tone: 'warn', icon: Video })
  if (plan?.os_hint && plan.os_hint !== 'unknown') chips.push({ key: 'os', label: plan.os_hint, tone: 'neutral', icon: Cpu })
  if (vm.inventory_source === 'kubevirt') chips.push({ key: 'k8s', label: vm.k8s_namespace ? `KubeVirt · ${vm.k8s_namespace}` : 'KubeVirt', tone: 'info', icon: Cloud })
  if (vm.managed === false) chips.push({ key: 'disc', label: 'Discovered', tone: 'warn', icon: Lock, title: 'Not managed by Machina yet' })
  if (vm.project) chips.push({ key: 'proj', label: vm.project, tone: 'neutral', icon: Server })
  return chips
}

export default function VmFeatureChips({ vm, plan }: { vm: PlatformVm; plan: ConsoleHubPlan | null }) {
  const chips = vmFeatures(vm, plan)
  if (chips.length === 0) return null
  return (
    <div className="flex flex-wrap gap-1.5" data-testid="vm-feature-chips">
      {chips.map(({ key, label, tone, icon: Icon, title }) => (
        <span key={key} title={title ?? label} className={`inline-flex items-center gap-1 text-[11px] font-medium capitalize ${statusPillClasses(tone)}`}>
          <Icon className="w-3 h-3" aria-hidden />
          {label}
        </span>
      ))}
    </div>
  )
}

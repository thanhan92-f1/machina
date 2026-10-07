// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { describe, expect, it } from 'vitest'
import { appendVmConnection, sanitizeVmInfo, vmConsoleRoute, vmDetailRoute, vmScopeParam } from './vm'
import { buildFluxvmCreateRequest } from '../components/vm/FluxvmCreatePanel'

describe('FluxVM VM scope', () => {
  it('maps the fluxvm connection to backend=fluxvm on API URLs', () => {
    expect(vmScopeParam(undefined)).toBe('')
    expect(vmScopeParam('system')).toBe('')
    expect(vmScopeParam('session')).toBe('connection=session')
    expect(vmScopeParam('fluxvm')).toBe('backend=fluxvm')
    expect(appendVmConnection('/api/v1/vms/a/start', 'fluxvm')).toBe('/api/v1/vms/a/start?backend=fluxvm')
    expect(appendVmConnection('/api/v1/vms/a?x=1', 'fluxvm')).toBe('/api/v1/vms/a?x=1&backend=fluxvm')
  })

  it('keeps connection=fluxvm on in-app routes and sends the console to the serial tab', () => {
    expect(vmDetailRoute('a b', 'fluxvm')).toBe('/vms/a%20b?connection=fluxvm')
    expect(vmConsoleRoute('a', 'fluxvm')).toBe('/vms/a?connection=fluxvm&tab=serial')
    expect(vmConsoleRoute('a', 'session')).toBe('/vms/a/consolehub?connection=session')
    expect(vmConsoleRoute('a')).toBe('/vms/a/consolehub')
  })

  it('normalizes FluxVM list rows onto the fluxvm connection', () => {
    const vm = sanitizeVmInfo({ name: 'fc1', state: 'running', vcpus: 2, memory_mb: 1024, backend: 'fluxvm', fluxvm_backend: 'firecracker' })
    expect(vm).toMatchObject({ name: 'fc1', backend: 'fluxvm', fluxvm_backend: 'firecracker', libvirt_connection: 'fluxvm' })
    const lv = sanitizeVmInfo({ name: 'lv1', state: 'shutoff', vcpus: 1, memory_mb: 512 })
    expect(lv?.backend).toBeUndefined()
    expect(lv?.libvirt_connection).toBeUndefined()
  })
})

describe('buildFluxvmCreateRequest', () => {
  it('builds a fluxvm create payload and drops empty optionals', () => {
    const req = buildFluxvmCreateRequest({
      name: ' fc1 ',
      vcpus: 2,
      memoryMb: 1024,
      diskGb: 0,
      hypervisor: 'firecracker',
      image: ' /img/u.qcow2 ',
      bridge: '',
      cloudInitUser: ' ',
    })
    expect(req).toMatchObject({ name: 'fc1', backend: 'fluxvm', fluxvm_backend: 'firecracker', fluxvm_image: '/img/u.qcow2', disk_gb: 0 })
    expect(req.fluxvm_network).toBe('netns')
    expect(req.fluxvm_bridge).toBeUndefined()
    expect(req.cloud_init_user).toBeUndefined()
  })

  it('maps bridge and direct-uplink networks onto their own fields', () => {
    const base = { name: 'v', vcpus: 1, memoryMb: 512, diskGb: 0, hypervisor: 'qemu', image: '/i' }
    const br = buildFluxvmCreateRequest({ ...base, network: 'bridge', bridge: ' br0 ' })
    expect(br.fluxvm_bridge).toBe('br0')
    expect(br.fluxvm_network).toBeUndefined()
    const d = buildFluxvmCreateRequest({ ...base, network: 'direct', directUplink: 'eno1', directMode: 'l2-uplink', directGuestIps: '10.0.0.5, 10.0.0.6' })
    expect(d).toMatchObject({ fluxvm_direct_uplink: 'eno1', fluxvm_direct_mode: 'l2-uplink', fluxvm_direct_guest_ips: ['10.0.0.5', '10.0.0.6'] })
    expect(d.fluxvm_network).toBeUndefined()
  })
})

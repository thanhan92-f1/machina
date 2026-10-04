import { describe, expect, it } from 'vitest'
import { describeAllow, formatRemaining, groupQuarantines, type VmQuarantine } from './vmNetpol'

const q = (over: Partial<VmQuarantine>): VmQuarantine => ({
  vm: 'web-1',
  since: '2026-10-04T08:00:00Z',
  until: '2026-10-04T09:00:00Z',
  remaining_secs: 100,
  allow: [],
  taps: [],
  ...over,
})

describe('quarantine helpers', () => {
  it('folds a fleet quarantine held by every host into one row per VM', () => {
    const rows = groupQuarantines([
      q({ hostname: 'hv1', taps: [] }),
      q({ hostname: 'hv2', taps: ['vnet3'], remaining_secs: 120, until: '2026-10-04T09:00:02Z' }),
      q({ vm: 'db-1', hostname: 'hv1', taps: ['vnet1'] }),
    ])
    expect(rows.map((r) => r.vm)).toEqual(['db-1', 'web-1'])
    expect(rows[1].hosts).toEqual(['hv2'])
    expect(rows[1].taps).toEqual(['vnet3'])
    expect(rows[1].remaining_secs).toBe(120)
    expect(rows[1].until).toBe('2026-10-04T09:00:02Z')
  })

  it('formats remaining time and exceptions', () => {
    expect(formatRemaining(0)).toBe('expiring')
    expect(formatRemaining(42)).toBe('42s left')
    expect(formatRemaining(600)).toBe('10m left')
    expect(formatRemaining(3 * 3600 + 120)).toBe('3h 2m left')
    expect(describeAllow({ direction: 'ingress', peer: 'host', proto: 'tcp', port: 22 })).toBe('ingress host tcp/22')
    expect(describeAllow({ direction: 'egress', peer: 'world' })).toBe('egress world any')
  })
})

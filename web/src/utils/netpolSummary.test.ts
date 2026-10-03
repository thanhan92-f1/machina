// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { describe, expect, it } from 'vitest'
import { selectorText, summarizeSpec } from './netpolSummary'
import { flowQuery, labelError } from '../api/vmNetpol'
import { filterToArgs, parseFlowArgs } from '../components/flow/FlowTerminal'

describe('summarizeSpec', () => {
  it('describes allow, deny, empty rules and default deny', () => {
    const s = summarizeSpec({
      endpointSelector: { matchLabels: { 'k8s:app': 'db' } },
      ingress: [
        { fromEndpoints: [{ matchLabels: { app: 'web' } }], toPorts: [{ ports: [{ port: '5432', protocol: 'TCP' }] }] },
        { toPorts: [{ ports: [{ port: '9100', endPort: 9102, protocol: 'TCP' }] }] },
      ],
      egress: [{}],
      egressDeny: [{ toCIDRSet: [{ cidr: '10.0.0.0/8', except: ['10.1.0.0/16'] }] }],
    })
    expect(s.subject).toBe('app=db')
    expect(s.rules[0]).toMatchObject({ direction: 'ingress', deny: false, text: 'from VMs app=web on 5432/TCP' })
    expect(s.rules[1].text).toBe('from any peer on 9100-9102/TCP')
    expect(s.rules[2]).toMatchObject({ direction: 'egress', allowNothing: true })
    expect(s.rules[3]).toMatchObject({ deny: true, text: 'to 10.0.0.0/8 except 10.1.0.0/16' })
    expect(s.defaultDeny).toEqual(['ingress', 'egress'])
  })

  it('honours enableDefaultDeny and ICMP / entities', () => {
    const s = summarizeSpec({
      endpointSelector: {},
      enableDefaultDeny: { ingress: false },
      ingress: [{ fromEntities: ['host'], icmps: [{ fields: [{ type: 'EchoRequest', family: 'IPv4' }] }] }],
    })
    expect(s.subject).toBe('all VMs')
    expect(s.rules[0].text).toBe('from entity:host on ICMP EchoRequest')
    expect(s.defaultDeny).toEqual([])
  })

  it('renders match expressions', () => {
    expect(selectorText({ matchExpressions: [{ key: 'role', operator: 'In', values: ['a', 'b'] }, { key: 'x', operator: 'Exists' }] })).toBe(
      'role in (a, b), has x',
    )
  })
})

describe('flow filter flags', () => {
  it('parses hubble-style flags and round-trips', () => {
    const { filter, unknown } = parseFlowArgs('--vm web-1 --verdict=DROPPED --to db --port 5432')
    expect(unknown).toEqual([])
    expect(filter).toEqual({ vm: 'web-1', verdict: 'DROPPED', to_vm: 'db', port: '5432' })
    expect(parseFlowArgs(filterToArgs(filter)).filter).toEqual(filter)
  })

  it('reports unknown flags', () => {
    expect(parseFlowArgs('--nope 1 stray').unknown).toEqual(['--nope', 'stray'])
  })

  it('builds query strings without empty values', () => {
    expect(flowQuery({ vm: 'a b', verdict: '' }, { limit: 5 })).toBe('?vm=a+b&limit=5')
    expect(flowQuery({})).toBe('')
  })
})

describe('labelError', () => {
  it('accepts kubernetes label syntax', () => {
    expect(labelError('app', 'web')).toBeNull()
    expect(labelError('machina.io/tier', 'front-end_1')).toBeNull()
    expect(labelError('flag', '')).toBeNull()
  })
  it('rejects bad keys and values', () => {
    expect(labelError('', 'x')).not.toBeNull()
    expect(labelError('-bad', 'x')).not.toBeNull()
    expect(labelError('a/b/c', 'x')).not.toBeNull()
    expect(labelError('app', 'has space')).not.toBeNull()
  })
})

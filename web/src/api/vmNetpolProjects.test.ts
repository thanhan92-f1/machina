import { afterEach, describe, expect, it, vi } from 'vitest'
import { describeEgress, evidenceFilename, getEvidence, setProject, splitPorts, type ProjectNet } from './vmNetpol'

const net = (over: Partial<ProjectNet>): ProjectNet => ({
  project: 'shop',
  isolation: 'inherit',
  allow_host: true,
  egress_restricted: false,
  egress_allow: [],
  egress_ips: {},
  ...over,
})

describe('project networking helpers', () => {
  it('describes egress allowlists', () => {
    expect(describeEgress(net({}))).toBe('Any destination')
    expect(describeEgress(net({ egress_restricted: true }))).toBe('Only the project, host and DNS')
    expect(
      describeEgress(net({ egress_restricted: true, egress_allow: [{ to: '*.stripe.com', ports: ['443'] }, { to: '10.0.0.0/8' }] })),
    ).toBe('*.stripe.com (443), 10.0.0.0/8')
  })

  it('splits port lists', () => {
    expect(splitPorts('443, 80  53/udp')).toEqual(['443', '80', '53/udp'])
    expect(splitPorts('  ')).toEqual([])
  })

  it('names evidence files by UTC time', () => {
    const at = new Date('2026-10-04T13:05:09Z')
    expect(evidenceFilename('json', at)).toBe('segmentation-evidence-20261004T130509.json')
    expect(evidenceFilename('md', at)).toBe('segmentation-evidence-20261004T130509.md')
  })
})

describe('project networking requests', () => {
  afterEach(() => vi.unstubAllGlobals())

  it('puts project settings to the controller and fetches evidence verbatim', async () => {
    const calls: Array<{ url: string; method?: string; body?: unknown }> = []
    const raw = '{"kind":"machina.io/segmentation-evidence/v1","digest":"ab"}'
    vi.stubGlobal('fetch', async (url: string, init?: RequestInit) => {
      calls.push({ url, method: init?.method, body: init?.body ? JSON.parse(String(init.body)) : undefined })
      if (url.includes('/evidence')) return new Response(raw, { headers: { 'content-type': 'application/json' } })
      return new Response(JSON.stringify({ project: net({ isolation: 'isolated' }) }), { headers: { 'content-type': 'application/json' } })
    })
    const { project, ...rest } = net({ isolation: 'isolated' })
    await setProject(project, rest)
    expect(calls[0].url).toMatch(/platform\/controller\/api\/v1\/vm-network-policies\/projects\/shop$/)
    expect(calls[0].method).toBe('PUT')
    expect(calls[0].body).toMatchObject({ isolation: 'isolated', allow_host: true })
    expect(await getEvidence('host', 'json')).toBe(raw)
    expect(calls[1].url).toBe('/api/v1/vm-network-policies/evidence')
    await getEvidence('fleet', 'md')
    expect(calls[2].url).toMatch(/platform\/controller\/api\/v1\/vm-network-policies\/evidence\?format=md$/)
  })
})

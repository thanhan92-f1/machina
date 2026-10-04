// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { test, expect, type Page, type Route } from '@playwright/test'
import { mockPlatformApi } from './platformMock'

const PAGE = '/platform/zyra/security/network-policies'

const dbPolicy = {
  name: 'db-from-web',
  kind: 'CiliumNetworkPolicy',
  description: 'Only web VMs reach the database',
  labels: {},
  annotations: {},
  specs: [
    {
      endpointSelector: { matchLabels: { app: 'db' } },
      ingress: [{ fromEndpoints: [{ matchLabels: { app: 'web' } }], toPorts: [{ ports: [{ port: '5432', protocol: 'TCP' }] }] }],
    },
  ],
  yaml: 'apiVersion: cilium.io/v2\nkind: CiliumNetworkPolicy\nmetadata:\n  name: db-from-web\n',
  selected_vms: ['db-1'],
  enabled: true,
}

const endpoints = [
  { name: 'web-1', host: 'host-1', identity: 4101, labels: { app: 'web' }, addresses: ['10.0.0.5'], ingress_enforced: false, egress_enforced: true, policies: ['web-to-services'] },
  { name: 'db-1', host: 'host-1', identity: 4102, labels: { app: 'db' }, addresses: ['10.0.0.9'], ingress_enforced: true, egress_enforced: false, policies: ['db-from-web'] },
]

const flow = {
  ts: '2026-10-04T08:00:00Z',
  iface: 'vnet0',
  vm: 'web-1',
  direction: 'egress',
  src: '10.0.0.5',
  src_port: 40100,
  dst: '10.0.0.9',
  dst_port: 22,
  src_vm: 'web-1',
  dst_vm: 'db-1',
  src_labels: { app: 'web' },
  dst_labels: { app: 'db' },
  src_identity: 4101,
  dst_identity: 4102,
  proto: 'TCP',
  tcp_flags: 'SYN',
  bytes: 60,
  verdict: 'DROPPED',
  drop_reason: 'policy-denied',
  policy: 'db-from-web',
}

const json = (route: Route, body: unknown, status = 200) =>
  route.fulfill({ status, contentType: 'application/json', body: JSON.stringify(body) })

async function mockNetpol(page: Page) {
  const policies = [dbPolicy]
  const applied: string[] = []
  await page.route('**/flows/stream**', (route) =>
    route.fulfill({
      status: 200,
      headers: { 'content-type': 'text/event-stream', 'cache-control': 'no-cache' },
      body: `event: flow\ndata: ${JSON.stringify(flow)}\n\n`,
    }),
  )
  await page.route('**/vm-network-policies**', async (route) => {
    const req = route.request()
    const path = new URL(req.url()).pathname.replace(/^.*\/vm-network-policies/, '')
    const method = req.method()
    if (path === '' && method === 'GET') return json(route, { items: policies, warnings: [] })
    if (path === '' && method === 'POST') {
      const yaml = String((req.postDataJSON() as { yaml?: string }).yaml ?? '')
      const name = /name:\s*(\S+)/.exec(yaml)?.[1] ?? 'unnamed'
      applied.push(name)
      if (!policies.some((p) => p.name === name)) {
        policies.push({ ...dbPolicy, name, description: 'Web VMs reach services', selected_vms: ['web-1'], yaml })
      }
      return json(route, { applied: [name], warnings: [], sync: { ok: true, vms: 2, rules: 3, peers: 1, warnings: [] } })
    }
    if (path === '/validate') {
      const yaml = String((req.postDataJSON() as { yaml?: string }).yaml ?? '')
      const services = yaml.includes('toServices')
      return json(route, {
        valid: true,
        errors: [],
        warnings: [],
        compile_warnings: services ? ['web-to-services spec.egress[1]: toServices[0] selects no service'] : [],
        policies: [
          {
            ...dbPolicy,
            name: services ? 'web-to-services' : 'db-from-web',
            selected_vms: services ? ['web-1'] : ['db-1'],
            specs: services
              ? [{ endpointSelector: { matchLabels: { app: 'web' } }, egress: [{ toServices: [{ k8sService: { serviceName: 'shop-lb', namespace: 'shop' } }] }] }]
              : dbPolicy.specs,
          },
        ],
        rules: services ? 2 : 1,
        endpoints,
        selectors: [{ policy: 'web-to-services', path: 'spec.endpointSelector', selector: 'app=web', vms: ['web-1'] }],
      })
    }
    if (path === '/trace') {
      const q = req.postDataJSON() as { from: string; to: string; port?: number }
      const allowed = q.port === 5432
      return json(route, {
        allowed,
        from: { input: q.from, vm: q.from, identity: 4101, kind: 'vm' },
        to: { input: q.to, vm: q.to, identity: 4102, kind: 'vm' },
        protocol: 'TCP',
        port: q.port ?? 0,
        egress: { direction: 'egress', vm: q.from, enforced: false, verdict: 'no-policy' },
        ingress: allowed
          ? { direction: 'ingress', vm: q.to, enforced: true, verdict: 'allowed', rule: 'db-from-web spec.ingress[0]' }
          : { direction: 'ingress', vm: q.to, enforced: true, verdict: 'default-deny' },
        summary: allowed ? 'ALLOWED' : `DENIED: default deny at ingress of ${q.to}`,
      })
    }
    if (path === '/status') {
      return json(route, {
        policies: policies.length,
        managed_by: 'local',
        last_sync: { at: '2026-10-04T08:00:00Z', ok: true, vms: 2, rules: 3, peers: 1, warnings: [] },
        edge: { owner: 'daemon', enforcing: false, flow_log: true, taps: [] },
        enforcement: { mode: 'observe' },
        bpfd_available: true,
        cilium: null,
      })
    }
    if (path === '/endpoints') return json(route, { items: endpoints })
    if (path === '/selectors') return json(route, { items: [{ policy: 'db-from-web', path: 'spec.endpointSelector', selector: 'app=db', vms: ['db-1'] }] })
    if (path === '/fqdn-cache') return json(route, { items: [] })
    if (path === '/auth') return json(route, { items: [] })
    if (method === 'DELETE') {
      const name = decodeURIComponent(path.slice(1))
      policies.splice(policies.findIndex((p) => p.name === name), 1)
      return json(route, { deleted: name })
    }
    return json(route, { error: `unmocked ${method} ${path}` }, 404)
  })
  return { applied }
}

test('VM network policies: list, metrics and policy detail', async ({ page }) => {
  await mockPlatformApi(page, { tier: 'power' })
  await mockNetpol(page)
  await page.goto(PAGE)
  await expect(page.getByRole('heading', { name: 'VM Network Policies' })).toBeVisible({ timeout: 15_000 })
  const table = page.getByRole('table', { name: 'VM network policies' })
  await expect(table.getByText('db-from-web')).toBeVisible()
  await expect(table.getByText('db-1')).toBeVisible()
  await expect(page.getByText('Absent · native')).toBeVisible()
  await expect(page.getByText('Observe (audit)')).toBeVisible()
  await table.getByRole('button', { name: 'db-from-web' }).click()
  await expect(page.getByText(/from VMs app=web on 5432\/TCP/)).toBeVisible()
})

test('VM network policies: toServices template validates and applies', async ({ page }) => {
  await mockPlatformApi(page, { tier: 'power' })
  const m = await mockNetpol(page)
  await page.goto(`${PAGE}?tab=editor`)
  await expect(page.getByText(/toServices and authentication are enforced natively/)).toBeVisible({ timeout: 15_000 })
  await page.getByLabel('Template').selectOption('services')
  await expect(page.getByLabel('Policy YAML')).toHaveValue(/k8sService:/)
  await page.getByRole('button', { name: 'Validate & preview' }).click()
  await expect(page.getByText('Valid', { exact: true })).toBeVisible()
  await expect(page.getByText(/selects no service/)).toBeVisible()
  await expect(page.getByText(/service:shop\/shop-lb/)).toBeVisible()
  await page.getByRole('button', { name: 'Apply' }).click()
  await expect(page.getByText(/Applied web-to-services/)).toBeVisible()
  await expect(page.getByRole('table', { name: 'VM network policies' }).getByText('web-to-services')).toBeVisible()
  expect(m.applied).toEqual(['web-to-services'])
})

test('VM network policies: policy tester and endpoints', async ({ page }) => {
  await mockPlatformApi(page, { tier: 'power' })
  await mockNetpol(page)
  await page.goto(`${PAGE}?tab=tester`)
  await page.locator('#tr-from').fill('web-1')
  await page.locator('#tr-to').fill('db-1')
  await page.locator('#tr-port').fill('5432')
  await page.getByRole('button', { name: 'Trace' }).click()
  await expect(page.getByText('ALLOWED', { exact: true })).toBeVisible()
  await expect(page.getByText(/db-from-web spec\.ingress\[0\]/)).toBeVisible()
  await page.locator('#tr-port').fill('22')
  await page.getByRole('button', { name: 'Trace' }).click()
  await expect(page.getByText('DENIED: default deny at ingress of db-1')).toBeVisible()
  await expect(page.getByText('default-deny', { exact: true })).toBeVisible()

  await page.goto(`${PAGE}?tab=endpoints`)
  const ep = page.getByRole('table', { name: 'Policy endpoints' })
  await expect(ep.getByText('web-1')).toBeVisible({ timeout: 15_000 })
  await expect(ep.getByText('4102')).toBeVisible()
})

test('VM network policies: flow terminal streams a dropped flow', async ({ page }) => {
  await mockPlatformApi(page, { tier: 'power' })
  await mockNetpol(page)
  await page.goto(`${PAGE}?tab=flows`)
  const term = page.getByLabel('Packet flow terminal')
  await expect(term).toBeVisible({ timeout: 15_000 })
  await expect(term.getByText(/machinactl flow observe --follow/)).toBeVisible()
  await expect(term.getByText(/DROPPED/).first()).toBeVisible({ timeout: 10_000 })
  await expect(term.getByText(/web-1/).first()).toBeVisible()
})

test('VM network policies: delete asks for confirmation', async ({ page }) => {
  await mockPlatformApi(page, { tier: 'power' })
  await mockNetpol(page)
  await page.goto(PAGE)
  const table = page.getByRole('table', { name: 'VM network policies' })
  await expect(table.getByText('db-from-web')).toBeVisible({ timeout: 15_000 })
  page.once('dialog', (d) => void d.accept())
  await table.getByRole('button', { name: 'Delete' }).click()
  await expect(page.getByText('Deleted db-from-web')).toBeVisible()
  await expect(page.getByText(/No VM network policies/)).toBeVisible()
})

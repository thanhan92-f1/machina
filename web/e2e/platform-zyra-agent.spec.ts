// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { test, expect } from '@playwright/test'
import { mockPlatformApi } from './platformMock'

const RUN = {
  answer: 'Two running machines have no completed backup: db-01 and web-01. I queued a backup for each.',
  steps: [
    { kind: 'tool_call', tool: 'list_vms', detail: '{"state":"running"}' },
    { kind: 'tool_result', tool: 'list_vms', detail: '{}' },
    { kind: 'tool_call', tool: 'propose_action', detail: '{"action":"create_backup","vm":"db-01"}' },
    { kind: 'tool_call', tool: 'propose_action', detail: '{"action":"create_backup","vm":"web-01"}' },
  ],
  proposed_action_ids: ['a', 'b'],
  provider: 'anthropic',
}

test.describe('Zyra agent panel', () => {
  test.beforeEach(async ({ page }) => {
    await mockPlatformApi(page)
  })

  test('asking shows the answer, the queued-proposals note and the step trace', async ({ page }) => {
    await page.route('**/ai/agent/run', (route) => route.fulfill({ json: RUN }))
    await page.goto('/platform/zyra/approvals')
    const panel = page.getByTestId('zyra-agent-panel')
    await expect(panel).toBeVisible({ timeout: 15_000 })
    await panel.getByText('Which machines have no backup?').click()
    await expect(panel.getByText(/db-01 and web-01/)).toBeVisible()
    await expect(panel.getByText('2 proposals added to the queue below')).toBeVisible()
    await panel.getByText(/How I got there/).click()
    await expect(panel.getByText('Queued a proposal').first()).toBeVisible()
  })

  test('a disabled AI provider shows the server message, not a blank panel', async ({ page }) => {
    await page.route('**/ai/agent/run', (route) =>
      route.fulfill({ status: 400, json: { error: 'Zyra AI is turned off — enable an AI provider in Settings → AI Providers' } }))
    await page.goto('/platform/zyra/approvals')
    const panel = page.getByTestId('zyra-agent-panel')
    await panel.getByText('Is anything unhealthy right now?').click()
    await expect(panel.getByRole('alert')).toContainText('turned off')
  })
})

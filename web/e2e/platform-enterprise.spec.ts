// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { test, expect } from '@playwright/test'
import { mockPlatformApi } from './platformMock'

test('Enterprise Keychain tab shows secrets inventory', async ({ page }) => {
  await mockPlatformApi(page, { tier: 'advanced' })
  await page.goto('/platform/enterprise?tab=keychain')
  await expect(page.getByRole('heading', { name: 'Enterprise Security' })).toBeVisible({ timeout: 15_000 })
  await expect(page.getByText('corp-vault')).toBeVisible()
  await expect(page.getByRole('heading', { name: 'Secrets inventory' })).toBeVisible()
})

test('Vault sync failure shows ErrorBanner', async ({ page }) => {
  await mockPlatformApi(page, { tier: 'advanced' })
  await page.goto('/platform/enterprise?tab=vault')
  await page.getByRole('button', { name: 'Sync all' }).click()
  await expect(page.getByRole('alert')).toContainText(/vault sync failed|failed/i, { timeout: 10_000 })
})

test('Vault "Sync all" surfaces a simulated-probe warning, not just the aggregate summary', async ({ page }) => {
  // syncOne() already showed the per-provider "(simulated probe)" message via toast — "Sync all"
  // only showed the aggregate summary ("Synced N provider(s)"), so a simulated Vault could look
  // fully live to anyone who only ever uses the bulk button.
  await mockPlatformApi(page, { tier: 'advanced' })
  await page.route('**/enterprise/vault/sync-all', async (route) => {
    return route.fulfill({
      json: {
        synced: 1,
        summary: 'Synced 1 provider(s) · 1 active',
        results: [{
          provider_id: 'v1', provider_name: 'corp-vault', status: 'active',
          message: 'Vault health OK at https://vault.corp:8200 (simulated probe)',
          last_sync_at: new Date().toISOString(),
        }],
      },
    })
  })
  await page.goto('/platform/enterprise?tab=vault')
  await page.getByRole('button', { name: 'Sync all' }).click()
  await expect(page.getByText(/simulated probe.*corp-vault|corp-vault.*simulated probe/i)).toBeVisible({ timeout: 10_000 })
})

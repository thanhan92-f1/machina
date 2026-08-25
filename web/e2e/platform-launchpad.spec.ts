// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.

import { test, expect } from '@playwright/test'
import { mockPlatformApi } from './platformMock'

test.describe('Platform Launchpad', () => {
  test.beforeEach(async ({ page }) => {
    await mockPlatformApi(page, { tier: 'power' })
  })

  test('navigates to Launchpad and shows Grafana tile', async ({ page }) => {
    await page.goto('/platform/launchpad')
    await expect(page.getByTestId('platform-launchpad-page')).toBeVisible({ timeout: 15_000 })
    await expect(page.getByTestId('platform-launchpad-page').getByRole('heading', { name: 'Launchpad' })).toBeVisible()
    await expect(page.getByTestId('platform-launchpad-page').getByTestId('launchpad-tile-grafana').first()).toBeVisible()
    await expect(page.getByTestId('launchpad-hero-search')).toBeVisible()
    await expect(page.getByTestId('launchpad-spaces-grid')).toBeVisible()
  })

  test('Spotlight finds Grafana and offers open action', async ({ page }) => {
    await page.goto('/platform/launchpad')
    await page.getByLabel('Open Spotlight').click()
    const spotlightInput = page.locator('[aria-label="Zeus Spotlight"] input[type="text"]')
    await expect(spotlightInput).toBeVisible({ timeout: 10_000 })
    await Promise.all([
      page.waitForResponse((r) => r.url().includes('/api/v1/launchpad/search') && r.ok()),
      spotlightInput.fill('grafana'),
    ])
    await expect(page.getByText(/Monitoring · Healthy · Open/i)).toBeVisible({ timeout: 10_000 })
    await expect(page.getByText(/Monitoring · Healthy · Inspect/i)).toBeVisible()
  })

  test('app detail page shows human-first layout and inspector', async ({ page }) => {
    await page.goto('/platform/launchpad/apps/grafana')
    await expect(page.getByTestId('platform-launchpad-app-detail')).toBeVisible({ timeout: 15_000 })
    const detail = page.getByTestId('platform-launchpad-app-detail')
    await expect(detail.getByRole('heading', { name: 'Grafana' })).toBeVisible()
    await expect(detail.getByRole('button', { name: 'Inspect Route' })).toBeVisible()
    await detail.getByRole('button', { name: 'Inspect Route' }).click()
    await expect(page.getByTestId('launchpad-inspector')).toBeVisible({ timeout: 10_000 })
    await expect(page.getByTestId('launchpad-route-lens')).toBeVisible()
  })

  test('space drill-down lists monitoring apps', async ({ page }) => {
    await page.goto('/platform/launchpad/spaces/monitoring')
    await expect(page.getByTestId('platform-launchpad-space-monitoring')).toBeVisible({ timeout: 15_000 })
    await expect(page.getByTestId('launchpad-tile-grafana')).toBeVisible()
    await expect(page.getByTestId('launchpad-tile-prometheus-server')).toBeVisible()
  })
})

// Copyright (c) 2026 ZyvorAI Labs Private Limited. All rights reserved.
// Proprietary software — see LICENSE in the repository root.
// https://zyvor.dev · info@zyvor.dev

import { test, expect } from '@playwright/test'
import { mockAuthenticatedApi } from './helpers/authMock'

async function mockUnauthenticatedApi(page: import('@playwright/test').Page) {
  await page.route('**/api/v1/**', async (route) => {
    const url = route.request().url()
    if (url.includes('/auth/providers')) {
      return route.fulfill({
        json: {
          pam: { enabled: true },
          ldap: { enabled: false },
          oidc: { enabled: false, button_label: 'Sign in with SSO' },
        },
      })
    }
    if (url.includes('/auth/session')) {
      return route.fulfill({ status: 401, json: { error: 'unauthenticated' } })
    }
    return route.fulfill({ status: 401, json: { error: 'unauthenticated', error_code: 'unauthorized' } })
  })
}

test('login page shows PAM form when OIDC is off', async ({ page }) => {
  await mockUnauthenticatedApi(page)
  await page.goto('/login')
  await expect(page.getByText('Machina').first()).toBeVisible()
  await expect(page.getByLabel('Username')).toBeVisible()
})

test('authenticated /login redirects to dashboard', async ({ page }) => {
  await mockAuthenticatedApi(page)
  await page.goto('/login')
  await expect(page).toHaveURL('/', { timeout: 15_000 })
  await expect(page.locator('#main-content')).toBeVisible({ timeout: 15_000 })
})

test('VM list shows empty state when authenticated', async ({ page }) => {
  await mockAuthenticatedApi(page)
  await page.goto('/vms')
  await expect(page.getByRole('heading', { name: /virtual machines/i })).toBeVisible({ timeout: 15_000 })
})

test('Fleet page loads when authenticated', async ({ page }) => {
  await mockAuthenticatedApi(page)
  await page.goto('/fleet')
  await expect(page.getByRole('heading', { name: 'Fleet', exact: true })).toBeVisible({ timeout: 15_000 })
})

test('language switcher changes login label', async ({ page }) => {
  await mockUnauthenticatedApi(page)
  await page.goto('/login')
  await page.getByLabel('Language').selectOption('es')
  await expect(page.getByLabel('Usuario')).toBeVisible()
})

test('OpenStack instances shows sanitized error when API returns HTML', async ({ page }) => {
  await mockAuthenticatedApi(page)
  await page.goto('/openstack/instances')
  await expect(page.getByText(/Failed to load instances/i)).toBeVisible({ timeout: 15_000 })
  await expect(page.getByText(/HTML error page/i).first()).toBeVisible()
})

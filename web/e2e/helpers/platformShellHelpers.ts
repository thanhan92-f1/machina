// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: LicenseRef-Zyvor-Production-1.0

import { expect, type Page } from '@playwright/test'

/** Invisible full-screen layers that swallow clicks after partial dismiss. */
export async function assertNoShellClickBlockers(page: Page) {
  await expect(page.locator('.fixed.inset-0.z-40[aria-hidden="true"]')).toHaveCount(0)
  await expect(page.getByRole('dialog', { name: 'Mission Control' })).toHaveCount(0)
  await expect(page.locator('[aria-labelledby="dock-editor-title"]')).toHaveCount(0)
}

/**
 * Both primary navigation surfaces remain clickable after overlay churn: the sidebar
 * (`aside[aria-label="Sections"]`, off-canvas below 1025px or hidden via "Hide Sidebar") and the
 * top bar (`nav[aria-label="Primary"]` — a row of buttons that each open a `.gnb-flyout` of links,
 * not direct links themselves). The Mac dock this used to also exercise is gone by design (see
 * docs/design/APPLE-UX-CONTRACT.md — "do not reintroduce a dock").
 */
export async function assertShellNavResponsive(page: Page) {
  await assertNoShellClickBlockers(page)

  await expandAllSidebarSections(page)
  const sidebarLink = page.locator('aside[aria-label="Sections"] a[href^="/platform"]').first()
  if (await sidebarLink.isVisible().catch(() => false)) {
    const href = await sidebarLink.getAttribute('href')
    await sidebarLink.click()
    if (href) {
      await expect(page).toHaveURL(new RegExp(`${href.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(\\?|$)`))
    }
  }

  const groupButton = page.getByRole('navigation', { name: 'Primary' }).getByRole('button').first()
  await groupButton.click()
  const flyoutLink = page.locator('.gnb-flyout-link').first()
  await expect(flyoutLink).toBeVisible()
  await flyoutLink.click()
  await expect(page).toHaveURL(/\/platform/)
  await assertNoShellClickBlockers(page)
}

export async function clickRandomSidebar(page: Page) {
  await expandAllSidebarSections(page)
  const links = page.locator('aside[aria-label="Sections"] a[href^="/platform"]')
  const count = await links.count()
  if (count === 0) return
  const idx = Math.floor(Math.random() * count)
  const href = await links.nth(idx).getAttribute('href')
  await links.nth(idx).click()
  if (href) {
    await expect(page).toHaveURL(new RegExp(`${href.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}(\\?|$)`))
  }
}

/**
 * Sidebar sections are accordions (only the section containing the current route is open), so a link
 * may exist in a collapsed section. Open collapsed sections one by one until `href` is visible.
 */
export async function sidebarLink(page: Page, href: string) {
  const link = page.locator(`aside[aria-label="Sections"] a[href="${href}"]`)
  if (await link.isVisible().catch(() => false)) return link
  const collapsed = page.locator('aside[aria-label="Sections"] .platform-sidebar-section-header[aria-expanded="false"]')
  const n = await collapsed.count()
  for (let i = 0; i < n; i++) {
    await collapsed.first().click()
    if (await link.isVisible().catch(() => false)) break
  }
  return link
}

export async function expandAllSidebarSections(page: Page) {
  const collapsed = page.locator('aside[aria-label="Sections"] .platform-sidebar-section-header[aria-expanded="false"]')
  while ((await collapsed.count()) > 0) await collapsed.first().click()
}

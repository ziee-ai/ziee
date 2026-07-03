import { Page, expect } from '@playwright/test'
import { byTestId } from '../../testid.ts'

/**
 * Navigation helpers for the Skills E2E suite. Mirrors the shape of
 * `projects/helpers/project-helpers.ts` so reviewers familiar with
 * the projects tests can read these at a glance.
 *
 * Routes (from `src/modules/skill/module.tsx`):
 *   - user page:  `/settings/skills`                  (perm: skills::read)
 *   - admin page: `/settings/skills-admin`   (perm: skills::manage_system)
 *
 * Both pages render their title via `SettingsPageContainer`, which emits
 * an antd `<Title level={4}>` (an `<h4>`). The list page title is
 * exactly "Skills"; the admin page title is exactly "System Skills".
 */

// Don't wait for `networkidle` — the app shell keeps a long-lived
// realtime-sync SSE stream open, so the network never settles. Waiting
// for the page's distinctive heading is sufficient: by then the route
// component has mounted and the store hydration the next assertions
// need is already in flight.
export async function goToSkillsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/skills`)
  await byTestId(page, 'skills-page').waitFor({ timeout: 15000 })
}

export async function goToAdminSkillsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/skills-admin`)
  await byTestId(page, 'skills-admin-page').waitFor({ timeout: 15000 })
}

/**
 * Bump the skills list to its largest page size so an item that would
 * otherwise sort onto a later page (the list is `name ASC` + client-paginated
 * at 10, and the built-in capability skills already fill the first page) is
 * visible on a single page. No-op when the list is short enough that the
 * paginator isn't rendered.
 */
export async function showAllSkills(page: Page) {
  const sizeSelect = byTestId(page, 'skill-list-pagination-page-size')
  if (await sizeSelect.isVisible().catch(() => false)) {
    await sizeSelect.click()
    await byTestId(page, 'skill-list-pagination-page-size-opt-50').click()
  }
}

/**
 * Assert the user-facing skills list empty state. The empty-state copy
 * is rendered as an antd `<Empty>` description string (see
 * `SkillsList.tsx`).
 */
export async function assertSkillsEmptyState(page: Page) {
  await expect(byTestId(page, 'skill-list-empty')).toBeVisible()
}

/**
 * Assert the admin (system) skills list empty state — distinct copy
 * from the user list (see `admin/AdminSkillsPage.tsx`).
 */
export async function assertAdminSkillsEmptyState(page: Page) {
  await expect(byTestId(page, 'skill-admin-empty')).toBeVisible()
}

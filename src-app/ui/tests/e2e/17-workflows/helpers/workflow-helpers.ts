import { Page, expect } from '@playwright/test'

/**
 * Navigation helpers for the Workflows E2E suite. Mirrors the shape of
 * `11-projects/helpers/project-helpers.ts` so reviewers familiar with
 * the projects tests can read these at a glance.
 *
 * Routes (from `src/modules/workflow/module.tsx`):
 *   - user list:  `/workflows`                  (perm: workflows::read)
 *   - admin page: `/settings/workflows-admin`   (perm: workflows::manage_system)
 *
 * Both pages render their title via `SettingsPageContainer`, which emits
 * an antd `<Title level={4}>` (an `<h4>`). The list page title is
 * exactly "Workflows"; the admin page title is exactly "System Workflows".
 */

// Don't wait for `networkidle` — the app shell keeps a long-lived
// realtime-sync SSE stream open, so the network never settles. Waiting
// for the page's distinctive heading is sufficient: by then the route
// component has mounted and the store hydration the next assertions
// need is already in flight.
export async function goToWorkflowsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/workflows`)
  await page
    .getByRole('heading', { level: 4, name: 'Workflows', exact: true })
    .first()
    .waitFor({ timeout: 15000 })
}

export async function goToAdminWorkflowsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/workflows-admin`)
  await page
    .getByRole('heading', { level: 4, name: 'System Workflows', exact: true })
    .first()
    .waitFor({ timeout: 15000 })
}

/**
 * Assert the user-facing workflows list empty state. The empty-state
 * copy is rendered as an antd `<Empty>` description string (see
 * `WorkflowsList.tsx`).
 */
export async function assertWorkflowsEmptyState(page: Page) {
  await expect(
    page.getByText(/no workflows installed yet/i),
  ).toBeVisible()
}

/**
 * Assert the admin (system) workflows list empty state — distinct copy
 * from the user list (see `admin/AdminWorkflowsPage.tsx`).
 */
export async function assertAdminWorkflowsEmptyState(page: Page) {
  await expect(
    page.getByText(/no system workflows installed/i),
  ).toBeVisible()
}

import { APIRequestContext, Page, expect } from '@playwright/test'
import { gzipSync } from 'node:zlib'

/**
 * Navigation helpers for the Workflows E2E suite. Mirrors the shape of
 * `11-projects/helpers/project-helpers.ts` so reviewers familiar with
 * the projects tests can read these at a glance.
 *
 * Routes (from `src/modules/workflow/module.tsx`):
 *   - user list:  `/settings/workflows`         (perm: workflows::read; A2 moved it under settings)
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
  await page.goto(`${baseURL}/settings/workflows`)
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

// ─────────────────────────────────────────────────────────────────────────────
// Seed + run helpers for the standalone-runs E2E flow (A1/A4/A5/D2/D3).
//
// A2 moved the user list to `/settings/workflows`. These helpers seed a dev
// workflow through the API (a gzip+tar bundle posted to `/api/workflows/import`)
// so the list isn't empty, then drive the Run dialog + Runs tab in the browser.
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Navigate to the (settings-placed) user Workflows page and wait for its
 * heading. The route moved from `/workflows` to `/settings/workflows` (A2).
 */
export async function goToWorkflowsSettingsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/workflows`)
  await page
    .getByRole('heading', { level: 4, name: 'Workflows', exact: true })
    .first()
    .waitFor({ timeout: 15000 })
}

/**
 * Build a single-file gzip+tar bundle containing `workflow.yaml`. Implements
 * just enough of the POSIX ustar format for one small text entry (the backend's
 * tar reader accepts a standard ustar archive). Runs in Node (test process), so
 * `node:zlib` is available — unlike `page.evaluate`, which runs in the browser.
 */
export function buildWorkflowBundle(yaml: string): Buffer {
  const content = Buffer.from(yaml, 'utf8')
  // 512-byte ustar header.
  const header = Buffer.alloc(512)
  header.write('workflow.yaml', 0, 'utf8') // name
  header.write('0000644\0', 100, 'utf8') // mode
  header.write('0000000\0', 108, 'utf8') // uid
  header.write('0000000\0', 116, 'utf8') // gid
  header.write(content.length.toString(8).padStart(11, '0') + '\0', 124, 'utf8') // size (octal)
  header.write('00000000000\0', 136, 'utf8') // mtime
  header.write('0', 156, 'utf8') // typeflag (regular file)
  header.write('ustar\0', 257, 'utf8') // magic
  header.write('00', 263, 'utf8') // version
  // checksum: sum of all bytes with the checksum field treated as 8 spaces.
  for (let i = 148; i < 156; i++) header[i] = 0x20
  let sum = 0
  for (let i = 0; i < 512; i++) sum += header[i]
  header.write(sum.toString(8).padStart(6, '0') + '\0 ', 148, 'utf8')

  // Body padded to a 512-byte boundary, then two zero blocks (end-of-archive).
  const bodyPad = (512 - (content.length % 512)) % 512
  const tar = Buffer.concat([
    header,
    content,
    Buffer.alloc(bodyPad),
    Buffer.alloc(1024),
  ])
  return gzipSync(tar)
}

/**
 * Dev-import a workflow via `POST /api/workflows/import` (multipart). Returns
 * the created workflow id. The bundle is built in Node + posted via the
 * Playwright `request` (APIRequestContext) fixture, which also runs in Node.
 */
export async function seedDevWorkflow(
  request: APIRequestContext,
  apiURL: string,
  token: string,
  slug: string,
  yaml: string,
): Promise<string> {
  const bundle = buildWorkflowBundle(yaml)
  const resp = await request.post(
    `${apiURL}/api/workflows/import?name=${encodeURIComponent(slug)}`,
    {
      headers: { Authorization: `Bearer ${token}` },
      multipart: {
        bundle: {
          name: 'bundle.tar.gz',
          mimeType: 'application/gzip',
          buffer: bundle,
        },
      },
    },
  )
  expect(resp.status(), `dev import should 201: ${await resp.text()}`).toBe(201)
  const body = await resp.json()
  return body.id as string
}

/**
 * Open a workflow's detail drawer by clicking its card on the list page.
 * Cards carry `data-workflow-id`; we match on the visible name instead so the
 * selector reads naturally.
 */
export async function openWorkflowCard(page: Page, name: string) {
  await page.locator('.ant-card', { hasText: name }).first().click()
  // The drawer renders the workflow name as a level-5 heading.
  await expect(
    page.getByRole('heading', { name, exact: false }).first(),
  ).toBeVisible({ timeout: 15000 })
}

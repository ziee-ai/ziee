import { test, expect } from '../../fixtures/test-context'
import { byTestId } from '../testid'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

/**
 * E2E — realtime sync of the `AssistantTemplate` entity (everyone-audience).
 *
 * Admin handlers emit `SyncEntity::AssistantTemplate` on create/update/delete
 * (`assistant/handlers.rs:398,509,554`) to `Audience::everyone()`. No
 * 13-sync spec covered this entity (assistant-sync covers the owner-scoped
 * `Assistant`). This drives cross-window: a template mutation on device A
 * reflects on device B's Assistant Templates page WITHOUT reload.
 *
 * Run with --workers=1 (shared backend + DB), admin↔admin.
 */

const API = '/api/assistant-templates'

async function createTemplate(
  baseURL: string,
  token: string,
  name: string,
): Promise<string> {
  const res = await fetch(`${baseURL}${API}`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${token}`,
    },
    body: JSON.stringify({ name, description: 'sync e2e', enabled: true }),
  })
  if (!res.ok) throw new Error(`create template failed: ${res.status}`)
  return (await res.json()).id as string
}

async function gotoTemplates(page: import('@playwright/test').Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/assistant-templates`)
  await expect(
    byTestId(page, 'template-assistants-card'),
  ).toBeVisible({ timeout: 30000 })
}

test.describe('Realtime sync — assistant templates (everyone, cross-window)', () => {
  test('create / update / delete on device A reflect on device B', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await gotoTemplates(page, baseURL)
    const token = await getAdminToken(apiURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoTemplates(pageB, baseURL)

      // Create on A (via API) → both windows list it live.
      const name = `XSync Template ${Date.now()}`
      const id = await createTemplate(baseURL, token, name)
      await expect(byTestId(page, 'template-assistants-card').filter({ hasText: name })).toBeVisible({ timeout: 15_000 })
      await expect(byTestId(pageB, 'template-assistants-card').filter({ hasText: name })).toBeVisible({
        timeout: 15_000,
      })

      // Update → device B shows the new name.
      const renamed = `${name} (v2)`
      const upd = await fetch(`${baseURL}${API}/${id}`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
        body: JSON.stringify({ name: renamed }),
      })
      expect(upd.ok).toBeTruthy()
      await expect(byTestId(pageB, 'template-assistants-card').filter({ hasText: renamed })).toBeVisible({
        timeout: 15_000,
      })

      // Delete → device B drops it.
      const del = await fetch(`${baseURL}${API}/${id}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
      })
      expect(del.ok || del.status === 204).toBeTruthy()
      await expect(byTestId(pageB, 'template-assistants-card')).not.toContainText(renamed, { timeout: 15_000 })
    } finally {
      await ctxB.close()
    }
  })
})

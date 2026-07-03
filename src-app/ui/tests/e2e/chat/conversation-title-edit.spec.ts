import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'

// audit id all-a8948378a6c1 — editing a conversation title through the UI
// (TitleEditor.tsx) was untested. The header pencil opens an inline input;
// Enter saves via PUT /api/conversations/{id}. We drive the real flow and
// assert the new title renders AND survives a reload (real backend persistence).

async function seedConversation(apiURL: string, token: string, title: string): Promise<string> {
  const res = await fetch(`${apiURL}/api/conversations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
    body: JSON.stringify({ title }),
  })
  if (!res.ok) throw new Error(`seed conversation: ${res.status} ${await res.text()}`)
  return (await res.json()).id as string
}

test.describe('Conversation title editing', () => {
  test('edit the title inline and it persists across reload', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const convId = await seedConversation(apiURL, token, 'Original Title')

    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForLoadState('domcontentloaded')

    // Open the inline editor.
    await page.getByRole('button', { name: 'Edit conversation title' }).click()
    const input = page.getByPlaceholder('Enter conversation title')
    await expect(input).toBeVisible({ timeout: 10000 })
    await input.fill('Renamed Via UI')
    await input.press('Enter')

    // The header reflects the new title. Scope to the header heading — the
    // renamed title now also appears in the sidebar's recent-conversations
    // list, so a bare getByText would be a strict-mode (2-element) violation.
    await expect(
      page.getByRole('heading', { name: 'Renamed Via UI' }),
    ).toBeVisible({ timeout: 10000 })

    // Reload → the real backend persisted it (PUT /api/conversations/{id}).
    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForLoadState('domcontentloaded')
    await expect(
      page.getByRole('heading', { name: 'Renamed Via UI' }),
    ).toBeVisible({ timeout: 15000 })
    await expect(page.getByText('Original Title')).toHaveCount(0)
  })
})

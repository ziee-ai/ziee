import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// audit id all-dddb0ce3b16c — the "In project: <name>" chip in the chat header
// (projects/chat-extension/extension.tsx:162-168) is a clickable Tag that
// routes to /projects/{id}; the existing project-context spec only asserts the
// text is present. This drives the INTERACTION: click the chip and assert it
// navigates to the project detail page.
test.describe('Conversation project chip — interactive', () => {
  test('clicking the header chip navigates to the project', async ({ page, testInfra }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)

    const projName = `Chip Project ${Date.now()}`
    const proj = await (
      await fetch(`${apiURL}/api/projects`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ name: projName }),
      })
    ).json()
    const projectId = proj.id as string

    const conv = await (
      await fetch(`${apiURL}/api/conversations`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${token}` },
        body: JSON.stringify({ title: 'Conv in project', project_id: projectId }),
      })
    ).json()
    const convId = conv.id as string

    await page.goto(`${baseURL}/chat/${convId}`)
    await page.waitForLoadState('domcontentloaded')

    const chip = byTestId(page, 'project-header-chip-tag')
    await expect(chip).toBeVisible({ timeout: 30000 })
    await chip.click()

    await expect(page).toHaveURL(new RegExp(`/projects/${projectId}`), { timeout: 15000 })
  })
})

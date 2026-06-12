import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage, selectModelInDropdown } from './helpers/chat-helpers'
import { FILE_ASSETS, attachFileViaUI, openFileInPanel } from './helpers/file-panel-helpers'

/**
 * E2E for the file **version bar** in the right panel.
 *
 * Seeds a file with two versions (upload via UI → append v2 via the
 * `files_mcp` edit API), opens it in the panel, and verifies the version bar
 * appears, lets you view an older version, and restore it. A single-version
 * file must NOT show the bar.
 */

const MODEL_DISPLAY = 'GPT-4o Mini'

async function setupProviderAndModel(apiURL: string, adminToken: string) {
  const providerId = await createProviderViaAPI(apiURL, adminToken, 'OpenAI', 'openai')
  await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
  await createModelViaAPI(apiURL, adminToken, providerId, undefined, undefined, 'openai')
}

/** New chat → pick the model → attach a file → send. Returns the conversation id. */
async function startChatWithFile(page: Page, baseURL: string, asset: string): Promise<string> {
  await goToNewChatPage(page, baseURL)
  await selectModelInDropdown(page, MODEL_DISPLAY)
  await attachFileViaUI(page, asset)
  const textarea = page.locator('textarea[placeholder*="Type your message"]')
  await textarea.fill('here is the file')
  await page.getByRole('button', { name: 'Send message' }).click()
  await page.waitForURL(/\/chat\/[a-f0-9-]+/, { timeout: 15000 })
  return page.url().split('/chat/')[1]!
}

async function findFileId(apiURL: string, token: string, filename: string): Promise<string> {
  const res = await fetch(`${apiURL}/api/files?page=1&per_page=50`, {
    headers: { Authorization: `Bearer ${token}` },
  })
  const body = await res.json()
  const match = (body.files as Array<{ id: string; filename: string }>).find(
    (f) => f.filename === filename,
  )
  if (!match) throw new Error(`file ${filename} not found in library`)
  return match.id
}

/**
 * Append a version via the files_mcp `rewrite_file` tool. Uses a full rewrite
 * (not str-replace) so the seeding is independent of the fixture file's exact
 * contents — robust against test.md ever changing.
 */
async function appendVersionViaApi(
  apiURL: string,
  token: string,
  conversationId: string,
  fileId: string,
  content: string,
) {
  const res = await fetch(`${apiURL}/api/files/mcp`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${token}`,
      'Content-Type': 'application/json',
      'x-conversation-id': conversationId,
    },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id: 1,
      method: 'tools/call',
      params: { name: 'rewrite_file', arguments: { id: fileId, content } },
    }),
  })
  const body = await res.json()
  expect(body.error, `rewrite_file error: ${JSON.stringify(body.error)}`).toBeFalsy()
}

test.describe('file version bar', () => {
  test('shows version history + restore for a multi-version file', async ({ page, testInfra }) => {
    const { apiURL, baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    const conversationId = await startChatWithFile(page, baseURL, FILE_ASSETS.md)

    // Append a 2nd version of the attached markdown file (content-independent).
    const fileId = await findFileId(apiURL, adminToken, 'test.md')
    await appendVersionViaApi(apiURL, adminToken, conversationId, fileId, '# Edited v2\n\nsecond version\n')

    await openFileInPanel(page, 'test.md')

    // The version bar must appear (the file now has 2 versions).
    const bar = page.locator('[data-testid="file-version-bar"]')
    await expect(bar).toBeVisible({ timeout: 10000 })

    // Switch to v1 → the Restore button appears.
    await page.locator('[data-testid="file-version-select"]').click()
    await page
      .locator('.ant-select-dropdown .ant-select-item-option', { hasText: 'v1' })
      .first()
      .click()
    await expect(page.locator('[data-testid="file-version-restore"]')).toBeVisible()

    // Restore v1 → appends a new head (v3). Wait for the restore to actually
    // complete: the bar's count tag must reach "3 versions" (the API call +
    // loadVersions refetch + re-render all finished). Asserting only
    // visibility would pass before the restore even started.
    await page.locator('[data-testid="file-version-restore"]').click()
    await expect(bar).toContainText('3 versions', { timeout: 10000 })
  })

  test('single-version file shows no version bar', async ({ page, testInfra }) => {
    const { apiURL, baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    await setupProviderAndModel(apiURL, adminToken)

    await startChatWithFile(page, baseURL, FILE_ASSETS.txt)
    await openFileInPanel(page, 'test.txt')

    // No edits → exactly one version → the bar is hidden.
    await expect(page.locator('[data-testid="file-version-bar"]')).toHaveCount(0)
  })
})

// Run with --workers=1 (mandated for all E2E here): parallel workers share the
// backend + test DB and race.
import path from 'path'
import { fileURLToPath } from 'url'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import { goToNewChatPage } from './helpers/chat-helpers'
import { attachFileViaUI, waitForFileInPreview } from './helpers/file-panel-helpers'

/**
 * frontend — file attachment through the chat composer.
 *
 * The composer's "+" dropdown uploads a file through the REAL upload pipeline
 * (no mocked upload — the mock fixture's bytes would never produce real chunk
 * provenance), and the resulting FileCard appears in the FilePreviewList. This
 * exercises the attach → upload-complete → preview flow end-to-end, including
 * attaching MULTIPLE files, which had no dedicated E2E coverage. A `local`
 * provider/model makes the chat page usable without depending on a remote
 * provider API key in the E2E env.
 */

const __dirname = path.dirname(fileURLToPath(import.meta.url))

const TXT_FIXTURE = path.resolve(
  __dirname,
  '../../../../server/tests/file/test_data/test.txt',
)
const TXT_FILENAME = 'test.txt'
const CSV_FIXTURE = path.resolve(
  __dirname,
  '../../../../server/tests/file/test_data/test.csv',
)
const CSV_FILENAME = 'test.csv'

test.describe('Chat - file attachment through the composer', () => {
  test('attaching multiple files shows their cards in the composer preview', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(
      apiURL,
      adminToken,
      'Local',
      'local',
    )
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      undefined,
      undefined,
      'local',
    )

    await goToNewChatPage(page, baseURL)

    // First attachment → its card appears in the preview list.
    await attachFileViaUI(page, TXT_FIXTURE)
    await waitForFileInPreview(page, TXT_FILENAME)

    // Second attachment → both cards coexist (the preview holds the full set).
    await attachFileViaUI(page, CSV_FIXTURE)
    await waitForFileInPreview(page, CSV_FILENAME)

    const txtCard = page.locator(
      `[data-testid="file-card"][data-filename="${TXT_FILENAME}"]`,
    )
    const csvCard = page.locator(
      `[data-testid="file-card"][data-filename="${CSV_FILENAME}"]`,
    )
    await expect(txtCard.first()).toBeVisible()
    await expect(csvCard.first()).toBeVisible()
  })
})

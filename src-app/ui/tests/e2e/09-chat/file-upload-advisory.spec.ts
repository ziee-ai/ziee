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
import { attachFileViaUI } from './helpers/file-panel-helpers'

/**
 * frontend-06 — upload-suitability advisory (FilePreviewList).
 *
 * The chat composer shows a non-blocking warning Alert for files the backend
 * annotates `processing_metadata.suitability='low'` at upload time (PowerPoint,
 * scanned/text-layer-less PDFs, archives, media). This was new UI with zero E2E
 * coverage. We upload a REAL low-suitability fixture (a .pptx — PowerPoint is
 * NOT text-extracted by the backend, so it reads "low") through the real upload
 * flow and assert the warning Alert renders with the filename + the suggestion
 * copy. We do NOT mock the upload — the mock fixture's processing_metadata is
 * null, which would never trigger the advisory.
 */

const __dirname = path.dirname(fileURLToPath(import.meta.url))

// PowerPoint fixture from the backend file-processing test data. PPTX is
// explicitly unsupported for text extraction (office.rs `can_process` returns
// false), so the upload pipeline annotates suitability='low' with the
// "PowerPoint reads poorly — export to PDF…" suggestion.
const PPTX_FIXTURE = path.resolve(
  __dirname,
  '../../../../server/tests/file/test_data/10_slides.pptx',
)
const PPTX_FILENAME = '10_slides.pptx'
// Substring of the backend suggestion copy for the PowerPoint branch
// (`file_suitability` in upload.rs).
const PPTX_SUGGESTION_SUBSTRING = 'PowerPoint reads poorly'

test.describe('Chat - File upload suitability advisory', () => {
  test('low-suitability upload renders a warning Alert with filename + suggestion', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    // A `local` provider needs no API key (main requires one for enabled REMOTE
    // providers) — this test only uploads a file + checks the advisory, it never
    // sends a message, so the model just has to exist to make the chat page
    // usable. Avoids depending on OPENAI_API_KEY in the E2E env.
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

    // Real upload through the + dropdown. Resolves once the file card appears
    // in the composer preview (FilePreviewList), which is where the advisory
    // Alert is rendered alongside it.
    await attachFileViaUI(page, PPTX_FIXTURE)

    // The advisory Alert renders for the low-suitability file (one per file,
    // keyed `file-preview-advisory-<id>`; scope to the one for this filename).
    const advisory = page.locator('[data-testid^="file-preview-advisory-"]').filter({
      hasText: PPTX_FILENAME,
    })
    await expect(advisory).toBeVisible({ timeout: 30000 })

    // The filename is bold (the <strong>) and the suggestion copy follows it.
    await expect(advisory.locator('strong')).toHaveText(PPTX_FILENAME)
    await expect(advisory).toContainText(PPTX_SUGGESTION_SUBSTRING)
  })
})

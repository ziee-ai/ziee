import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToNewChatPage,
  selectModelInDropdown,
  sendChatMessage,
} from '../09-chat/helpers/chat-helpers'
import { attachFileViaUI } from '../09-chat/helpers/file-panel-helpers'
import { promises as fs } from 'fs'
import os from 'os'
import path from 'path'

/**
 * E2E — the full Document-RAG user journey (audit gap all-3ec858ed548f).
 *
 * No spec covered the end-to-end flow across the THREE surfaces that make
 * up "RAG": (1) the admin enabling Document RAG deployment-wide, (2) a user
 * uploading a document into a conversation, and (3) the model answering a
 * question whose answer lives ONLY in that document. Every prior file-rag
 * spec stops at a single card's save round-trip; every chat spec uses a
 * pre-existing or mocked file. This stitches the journey together.
 *
 * Two layers, by design:
 *
 *  - The DETERMINISTIC layer (always runs): the admin flips Document RAG on
 *    deployment-wide (a real store→PUT /api/file-rag/admin-settings
 *    round-trip) and a user uploads a real document into the composer
 *    (real upload + server-side ingest — the file card only appears once
 *    the backend has accepted and stored the bytes). No mocks.
 *
 *  - The REAL-LLM retrieval layer (gated on ANTHROPIC_API_KEY): a real
 *    Anthropic model is asked a question answerable only from the uploaded
 *    document, and we assert the answer carries the unique beacon fact —
 *    proving the uploaded content is retrieved into the chat answer through
 *    the production path with NO mocks.
 *
 * Note on vector/semantic retrieval: a fresh test deployment has no
 * embedding-capable model registered (see embedding-section.spec.ts, which
 * asserts the "No embedding-capable models found." state), so the pgvector
 * semantic-search path cannot be exercised deterministically here. This
 * journey asserts document retrieval into the answer via the attach path;
 * the embedding-backed semantic ranking is covered by the backend
 * file_rag/embed integration tests. We do NOT fake an embedder.
 */

const HAS_ANTHROPIC_KEY = Boolean(process.env.ANTHROPIC_API_KEY)

// A unique fact that cannot be in any model's training data, so an echo of
// it can only have come from retrieving the uploaded document.
const BEACON = `ZIEE_RAG_BEACON_${Date.now()}`

async function writeBeaconDoc(): Promise<string> {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), 'ziee-rag-e2e-'))
  const file = path.join(dir, 'rag-beacon.txt')
  await fs.writeFile(
    file,
    [
      'Internal project memo — confidential.',
      '',
      `The classified project codename is ${BEACON}.`,
      'This codename appears nowhere else and must be retrieved from this document.',
      '',
      'End of memo.',
    ].join('\n'),
    'utf8',
  )
  return file
}

async function enableDocumentRagDeploymentWide(page: import('@playwright/test').Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/file-rag-admin`)

  await expect(byTestId(page, 'filerag-enable-card')).toBeVisible({ timeout: 30000 })

  const enableSwitch = byTestId(page, 'filerag-enable-switch')
  // Idempotent enable — only flip when currently off.
  if ((await enableSwitch.getAttribute('aria-checked')) !== 'true') {
    await enableSwitch.click()
  }

  const saveResp = page.waitForResponse(
    r =>
      r.url().includes('/api/file-rag/admin-settings') &&
      r.request().method() === 'PUT' &&
      r.status() === 200,
  )
  await byTestId(page, 'filerag-enable-save').click()
  await saveResp
}

test.describe('Document RAG — full journey (configure → upload → retrieve)', () => {
  test('admin enables Document RAG deployment-wide, then a document uploads into the composer', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // (1) Configure: turn Document RAG on deployment-wide (real PUT).
    await enableDocumentRagDeploymentWide(page, baseURL)

    // (2) Upload: a real document is ingested by the backend — the file
    // card only renders once the bytes are accepted and stored.
    const docPath = await writeBeaconDoc()
    await goToNewChatPage(page, baseURL)
    await attachFileViaUI(page, docPath)
    await expect(
      page.locator('[data-testid="file-card"][data-filename="rag-beacon.txt"]'),
    ).toBeVisible({ timeout: 30000 })
  })

  test('real model answers a question using only the uploaded document', async ({
    page,
    testInfra,
  }) => {
    test.skip(
      !HAS_ANTHROPIC_KEY,
      'ANTHROPIC_API_KEY not set — skipping the real-LLM retrieval leg',
    )
    test.slow()

    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const token = await page.evaluate(
      () => JSON.parse(localStorage.getItem('auth-storage')!).state.token,
    )
    const providerId = await createProviderViaAPI(apiURL, token, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, token, providerId)
    await createModelViaAPI(
      apiURL,
      token,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    // (1) Configure deployment-wide RAG.
    await enableDocumentRagDeploymentWide(page, baseURL)

    // (2) Upload the beacon document into a fresh conversation.
    const docPath = await writeBeaconDoc()
    await goToNewChatPage(page, baseURL)
    await selectModelInDropdown(page, 'Claude Haiku 4.5')
    await attachFileViaUI(page, docPath)
    await expect(
      page.locator('[data-testid="file-card"][data-filename="rag-beacon.txt"]'),
    ).toBeVisible({ timeout: 30000 })

    // (3) Retrieve: ask a question whose answer is only in the document.
    await sendChatMessage(
      page,
      'Read the attached memo and tell me the exact project codename it ' +
        'contains. Answer with the codename verbatim.',
      false,
    )

    // The answer must carry the beacon — proof the uploaded content was
    // retrieved end-to-end into the model's reply.
    await expect
      .poll(
        async () =>
          (await page.locator('[data-role="assistant"]').last().textContent()) ?? '',
        { timeout: 90000 },
      )
      .toContain(BEACON)
  })
})

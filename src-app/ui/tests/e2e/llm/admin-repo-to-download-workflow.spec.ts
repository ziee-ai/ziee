import { test } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { createRepository } from './helpers/repository-helpers'
import { createLocalProvider } from './helpers/provider-helpers'
import { clickProviderCard } from './helpers/navigation-helpers'
import { startModelDownload } from './helpers/model-helpers'

/**
 * E2E — a MULTI-STEP admin workflow chained in one session: configure a
 * repository → create a local provider → initiate a model download from a repo.
 * The existing specs cover these in isolation (llm-repositories is UI-only with
 * no download; the hub tests don't create repos). Only the external HuggingFace
 * download is mocked (the boundary); every UI step runs for real.
 *
 * (The optional 4th step — publishing the model to the hub — is a separate
 *  admin publish flow out of scope here.)
 */

test.describe('LLM — admin repo→provider→download multi-step workflow', () => {
  test('configure a repo, create a provider, and start a download', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Mock ONLY the HuggingFace download boundary so the flow is deterministic.
    const now = new Date().toISOString()
    await page.route(/\/api\/llm-models\/download$/, async (route, req) => {
      if (req.method() !== 'POST') return route.fallback()
      await route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({
          id: 'eeeeeeee-1111-2222-3333-444444444444',
          status: 'downloading',
          created_at: now,
          updated_at: now,
          request_data: { model_name: 'tiny-random-gpt2', display_name: 'WorkflowModel' },
        }),
      })
    })

    // Step 1 — configure a repository.
    const repoName = `wf-repo-${Date.now()}`
    await createRepository(page, baseURL, {
      name: repoName,
      url: `https://huggingface.co/${repoName}`,
      authType: 'none',
      enabled: true,
    })

    // Step 2 — create a local provider.
    const providerName = `wf-provider-${Date.now()}`
    await createLocalProvider(page, baseURL, providerName, 'multi-step workflow provider')
    await clickProviderCard(page, providerName)

    // Step 3 — initiate a model download from the seeded HuggingFace repo.
    await startModelDownload(page, {
      displayName: 'WorkflowModel',
      fileFormat: 'safetensors',
      engineType: 'mistralrs',
      chat: true,
      repositoryId: 'huggingface',
      repositoryPath: 'hf-internal-testing/tiny-random-gpt2',
      mainFilename: 'model.safetensors',
    })

    // Each step asserts its own success internally: createRepository waits for
    // 'Repository added successfully', and startModelDownload waits for
    // 'Download started successfully' — so reaching here means the full
    // configure-repo → create-provider → start-download chain completed.
    void repoName
  })
})

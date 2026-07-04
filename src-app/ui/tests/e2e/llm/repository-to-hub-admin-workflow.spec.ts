import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createRepository,
  goToRepositoriesPage,
  waitForRepositoriesPageLoad,
  toggleRepositoryStatus,
  assertRepositoryExists,
  assertRepositoryEnabled,
  assertRepositoryDisabled,
  deleteRepository,
} from './helpers/repository-helpers'
import {
  navigateToHub,
  waitForHubDataLoad,
} from '../hub/helpers/hub-navigation'

// 374a6efd — a MULTI-STEP admin workflow chained in ONE test, rather than the
// isolated single-step assertions the existing llm-repositories spec makes:
// configure an LLM repository (create → verify enabled → toggle disabled →
// re-enable), then cross into the downstream model surface (the Hub, where an
// admin browses/downloads models that a repository serves).
//
// The actual model-DOWNLOAD and hub-PUBLISH steps require a real engine/model
// mirror (gated like local-runtime/engine-lifecycle's ZIEE_E2E_ENGINE_MIRROR)
// and are covered by hub-download-* + hub-model-download-providers; this
// pins the configure→navigate admin chain that no single spec covered end-to-end.

test.describe('Admin workflow: configure repository → hub', () => {
  test('configure an LLM repository then reach the hub model surface', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const repositoryName = `wf-repo-${Date.now().toString(36)}`

    // Step 1 — configure a repository (enabled).
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: `https://huggingface.co/?r=${repositoryName}`,
      authType: 'none',
      enabled: true,
    })
    await assertRepositoryExists(page, repositoryName)
    await assertRepositoryEnabled(page, repositoryName)

    // Step 2 — toggle it off, then back on (an admin reconfiguring availability).
    await toggleRepositoryStatus(page, repositoryName)
    await assertRepositoryDisabled(page, repositoryName)
    await toggleRepositoryStatus(page, repositoryName)
    await assertRepositoryEnabled(page, repositoryName)

    // Step 3 — cross into the Hub (the downstream model-browse/download surface).
    await navigateToHub(page, baseURL, 'models')
    await waitForHubDataLoad(page)
    await expect(page).toHaveURL(/\/hub\/models/)

    // Cleanup so reruns don't accumulate repositories.
    await goToRepositoriesPage(page, baseURL)
    await waitForRepositoriesPageLoad(page)
    await deleteRepository(page, repositoryName)
  })
})

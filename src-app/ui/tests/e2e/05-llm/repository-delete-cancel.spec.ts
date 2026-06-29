import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  createRepository,
  openDeleteRepositoryDialog,
  cancelDeleteRepository,
  assertRepositoryExists,
} from './helpers/repository-helpers'

/**
 * E2E — LlmRepository delete Popconfirm CANCEL path (the cancelDeleteRepository
 * helper existed but was never exercised). Opening the delete confirmation and
 * clicking Cancel must dismiss it and leave the repository intact.
 */

test.describe('LLM Repositories — delete cancel', () => {
  test('cancelling the delete Popconfirm keeps the repository', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    const repositoryName = `cancel-del-repo-${Date.now()}`
    await createRepository(page, baseURL, {
      name: repositoryName,
      url: 'https://huggingface.co',
      authType: 'none',
      enabled: true,
    })
    await assertRepositoryExists(page, repositoryName)

    // Open the delete Popconfirm, then CANCEL.
    await openDeleteRepositoryDialog(page, repositoryName)
    await cancelDeleteRepository(page)

    // The confirm dialog closes and the repository is still present.
    await expect(page.locator('[data-testid^="llmrepo-delete-confirm-"]')).toHaveCount(0)
    await assertRepositoryExists(page, repositoryName)
  })
})

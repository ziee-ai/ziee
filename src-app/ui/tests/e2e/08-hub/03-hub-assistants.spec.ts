import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad } from './helpers/hub-navigation'
import {
  createAssistantFromHub,
  getAssistantCards,
  isAssistantCreated,
  getAssistantCardStatus,
} from './helpers/hub-assistants'
import { loginWithPerms } from '../permissions/fixtures'
import { Permissions } from '../../../src/api-client/types'

test.describe('Hub Assistants', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await navigateToHub(page, baseURL, 'assistants')
    await waitForHubDataLoad(page)
  })

  test('should display all hub assistants', async ({ page }) => {
    const assistantCards = await getAssistantCards(page)
    const count = await assistantCards.count()

    expect(count).toBeGreaterThan(0)
  })

  test('should show assistant cards with required information', async ({ page }) => {
    const assistantCards = await getAssistantCards(page)
    const firstCard = assistantCards.first()

    // Should have "Use Assistant" button
    await expect(firstCard.getByRole('button', { name: /use.*assistant/i })).toBeVisible()

    // Card should have content (text visible)
    await expect(firstCard).toContainText(/.+/)
  })

  // After page.reload() the Hub store re-fetches from the backend's
  // GET /api/hub/assistants, which stitches in the freshly-updated
  // created_ids. Previously these tests also called refreshHubData()
  // — that hits refresh-from-github and wipes out the local cache
  // including the stitched created_ids. Removed.

  test('should create assistant from hub without customization', async ({
    page,
    testInfra,
  }) => {
    const assistantCards = await getAssistantCards(page)
    const firstCard = assistantCards.first()

    // Get the assistant ID from the test ID
    const testId = await firstCard.getAttribute('data-testid')
    const assistantId = testId?.replace('hub-assistant-card-', '') || ''

    expect(assistantId).toBeTruthy()

    // Create assistant
    await createAssistantFromHub(page, assistantId)

    // Should show success message (use .first() to handle Ant Design duplicates)
    await expect(
      page.getByText(/created.*successfully|assistant.*created/i).first(),
    ).toBeVisible({ timeout: 5000 })

    // The card navigates away to /settings/assistants on success; go
    // back to the hub to verify the badge.
    await navigateToHub(page, testInfra.baseURL, 'assistants')
    await waitForHubDataLoad(page)

    const created = await isAssistantCreated(page, assistantId)
    expect(created).toBe(true)
  })

  test('should create assistant from hub with customization', async ({
    page,
    testInfra,
  }) => {
    const assistantCards = await getAssistantCards(page)

    // Get second assistant if available
    const count = await assistantCards.count()
    const cardIndex = count > 1 ? 1 : 0
    const card = assistantCards.nth(cardIndex)

    const testId = await card.getAttribute('data-testid')
    const assistantId = testId?.replace('hub-assistant-card-', '') || ''

    // Create with custom name
    const customName = `Custom Assistant ${Date.now()}`
    await createAssistantFromHub(page, assistantId, {
      name: customName,
      description: 'Custom description for testing',
    })

    // Should show success message (use .first() to handle Ant Design duplicates)
    await expect(
      page.getByText(/created.*successfully|assistant.*created/i).first(),
    ).toBeVisible({ timeout: 5000 })

    // Verify assistant was created
    await navigateToHub(page, testInfra.baseURL, 'assistants')
    await waitForHubDataLoad(page)

    const created = await isAssistantCreated(page, assistantId)
    expect(created).toBe(true)
  })

  test('should show "View" button for already created assistants', async ({
    page,
    testInfra,
  }) => {
    // Create first assistant
    const assistantCards = await getAssistantCards(page)
    const firstCard = assistantCards.first()

    const testId = await firstCard.getAttribute('data-testid')
    const assistantId = testId?.replace('hub-assistant-card-', '') || ''

    // Check if already created
    const alreadyCreated = await isAssistantCreated(page, assistantId)

    if (!alreadyCreated) {
      await createAssistantFromHub(page, assistantId)
      await navigateToHub(page, testInfra.baseURL, 'assistants')
      await waitForHubDataLoad(page)
    }

    // Should have "View" button instead of "Use"
    const card = page.getByTestId(`hub-assistant-card-${assistantId}`)
    await expect(card.getByRole('button', { name: /view/i })).toBeVisible()

    // "Use Assistant" is replaced by View once the assistant exists. But
    // "Use as Template" is a separate admin affordance gated on permissions
    // (not on the assistant), so it stays on the card.
    await expect(card.getByTestId('hub-assistant-use-btn')).toHaveCount(0)
    await expect(
      card.getByTestId('hub-assistant-use-as-template-btn'),
    ).toBeVisible()
  })

  test('should track creation status badge', async ({ page, testInfra }) => {
    const assistantCards = await getAssistantCards(page)
    const firstCard = assistantCards.first()

    const testId = await firstCard.getAttribute('data-testid')
    const assistantId = testId?.replace('hub-assistant-card-', '') || ''

    // Get initial status
    const initialStatus = await getAssistantCardStatus(page, assistantId)

    if (initialStatus === null) {
      // Not created yet, create it
      await createAssistantFromHub(page, assistantId)

      // Navigate back from /assistants to /hub/assistants and check
      await navigateToHub(page, testInfra.baseURL, 'assistants')
      await waitForHubDataLoad(page)

      const newStatus = await getAssistantCardStatus(page, assistantId)
      expect(newStatus).toBeTruthy()
      expect(newStatus).toMatch(/created/i)
    } else {
      // Already created
      expect(initialStatus).toMatch(/created/i)
    }
  })

  test('should navigate to assistant detail when clicking "View"', async ({
    page,
    testInfra,
  }) => {
    // Find an assistant that's already created
    const assistantCards = await getAssistantCards(page)
    let createdAssistantId = ''

    for (let i = 0; i < await assistantCards.count(); i++) {
      const card = assistantCards.nth(i)
      const testId = await card.getAttribute('data-testid')
      const assistantId = testId?.replace('hub-assistant-card-', '') || ''

      if (await isAssistantCreated(page, assistantId)) {
        createdAssistantId = assistantId
        break
      }
    }

    // If none created, create one first
    if (!createdAssistantId) {
      const firstCard = assistantCards.first()
      const testId = await firstCard.getAttribute('data-testid')
      createdAssistantId = testId?.replace('hub-assistant-card-', '') || ''

      await createAssistantFromHub(page, createdAssistantId)
      await navigateToHub(page, testInfra.baseURL, 'assistants')
      await waitForHubDataLoad(page)
    }

    // Click "View" button
    const card = page.getByTestId(`hub-assistant-card-${createdAssistantId}`)
    await card.getByRole('button', { name: /view/i }).click()

    // View navigates to /settings/assistants (the user's own assistants
    // list) per AssistantHubCard. Sanity-check by URL after navigation
    // settles, not waitForURL (SPA navigations don't always trip
    // its event hook reliably).
    await page.waitForLoadState('load').catch(() => {})
    const urlChanged = !page.url().includes('/hub/')
    const drawer = page.getByRole('dialog', { name: /assistant/i })
    const drawerVisible = await drawer.isVisible({ timeout: 2000 }).catch(() => false)

    expect(urlChanged || drawerVisible).toBe(true)
  })

  test('should prevent creation without required permissions', async ({
    page,
    testInfra,
  }) => {
    // User with hub::assistants::read but NOT ::create. Cards render
    // (read gives access) but AssistantHubCard's usePermission(
    // HubAssistantsCreate) hides the "Use Assistant" button.
    await loginWithPerms(
      page,
      testInfra.baseURL,
      testInfra.apiURL,
      [Permissions.HubAssistantsRead],
    )
    await navigateToHub(page, testInfra.baseURL, 'assistants')
    await waitForHubDataLoad(page)

    const cards = await getAssistantCards(page)
    const cardCount = await cards.count()
    if (cardCount > 0) {
      await expect(
        cards.first().getByRole('button', { name: /use.*assistant/i }),
      ).toHaveCount(0)
    }
  })
})

import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { navigateToHub, waitForHubDataLoad, refreshHubData } from './helpers/hub-navigation'
import {
  createAssistantFromHub,
  getAssistantCards,
  isAssistantCreated,
  getAssistantCardStatus,
} from './helpers/hub-assistants'

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

  test.skip('should create assistant from hub without customization', async ({ page }) => {
    // TODO: Requires "update hub data from platform server" feature
    // Currently, refreshHubData() only refreshes from GitHub, not from backend
    // This test will be enabled once the backend sync feature is implemented
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

    // Reload and check status
    await page.reload()
    await waitForHubDataLoad(page)
    await refreshHubData(page) // Refresh to get updated created_ids from backend
    await waitForHubDataLoad(page)

    const created = await isAssistantCreated(page, assistantId)
    expect(created).toBe(true)
  })

  test.skip('should create assistant from hub with customization', async ({ page }) => {
    // TODO: Requires "update hub data from platform server" feature
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
    await page.reload()
    await waitForHubDataLoad(page)
    await refreshHubData(page) // Refresh to get updated created_ids from backend
    await waitForHubDataLoad(page)

    const created = await isAssistantCreated(page, assistantId)
    expect(created).toBe(true)
  })

  test.skip('should show "View" button for already created assistants', async ({ page }) => {
    // TODO: Requires "update hub data from platform server" feature
    // Create first assistant
    const assistantCards = await getAssistantCards(page)
    const firstCard = assistantCards.first()

    const testId = await firstCard.getAttribute('data-testid')
    const assistantId = testId?.replace('hub-assistant-card-', '') || ''

    // Check if already created
    const alreadyCreated = await isAssistantCreated(page, assistantId)

    if (!alreadyCreated) {
      await createAssistantFromHub(page, assistantId)
      await page.reload()
      await waitForHubDataLoad(page)
      await refreshHubData(page) // Refresh to get updated created_ids from backend
      await waitForHubDataLoad(page)
    }

    // Should have "View" button instead of "Use"
    const card = page.getByTestId(`hub-assistant-card-${assistantId}`)
    await expect(card.getByRole('button', { name: /view/i })).toBeVisible()

    // Should NOT have "Use" button
    const useButton = card.getByRole('button', { name: /use/i })
    const useButtonVisible = await useButton.isVisible({ timeout: 1000 }).catch(() => false)
    expect(useButtonVisible).toBe(false)
  })

  test.skip('should track creation status badge', async ({ page }) => {
    // TODO: Requires "update hub data from platform server" feature
    const assistantCards = await getAssistantCards(page)
    const firstCard = assistantCards.first()

    const testId = await firstCard.getAttribute('data-testid')
    const assistantId = testId?.replace('hub-assistant-card-', '') || ''

    // Get initial status
    const initialStatus = await getAssistantCardStatus(page, assistantId)

    if (initialStatus === null) {
      // Not created yet, create it
      await createAssistantFromHub(page, assistantId)

      // Reload and check status
      await page.reload()
      await waitForHubDataLoad(page)
      await refreshHubData(page) // Refresh to get updated created_ids from backend
      await waitForHubDataLoad(page)

      const newStatus = await getAssistantCardStatus(page, assistantId)
      expect(newStatus).toBeTruthy()
      expect(newStatus).toMatch(/created/i)
    } else {
      // Already created
      expect(initialStatus).toMatch(/created/i)
    }
  })

  test.skip('should navigate to assistant detail when clicking "View"', async ({ page }) => {
    // TODO: Requires "update hub data from platform server" feature
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
      await page.reload()
      await waitForHubDataLoad(page)
    }

    // Click "View" button
    const card = page.getByTestId(`hub-assistant-card-${createdAssistantId}`)
    await card.getByRole('button', { name: /view/i }).click()

    // Should navigate to assistant detail or open drawer
    // Check for URL change or drawer opening
    const urlChanged = await page.waitForURL(/\/assistants\//, { timeout: 3000 }).catch(() => false)
    const drawer = page.getByRole('dialog', { name: /assistant/i })
    const drawerVisible = await drawer.isVisible({ timeout: 3000 }).catch(() => false)

    expect(urlChanged || drawerVisible).toBe(true)
  })

  test.skip('should prevent creation without required permissions', async ({ page }) => {
    // TODO: Implement test with user permission system
    // This requires creating a non-admin user without hub::assistants::create permission
  })
})

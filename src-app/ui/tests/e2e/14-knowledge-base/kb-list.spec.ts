import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// TEST-37 (ITEM-26,28,29,30): the Knowledge Base management surface —
// nav → /knowledge, empty state, create via the drawer, the card renders with a
// status chip, rename, and delete. (Document upload + retrieval-mode search are
// covered by kb-documents / kb-attach-chat specs.)
test.describe('Knowledge Base — list page', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/knowledge`)
    await expect(byTestId(page, 'kb-list-title')).toBeVisible()
  })

  test('passes accessibility checks', async ({ page }) => {
    await assertNoAccessibilityViolations(page)
  })

  test('shows the empty state and its create CTA', async ({ page }) => {
    await expect(byTestId(page, 'kb-list-empty')).toBeVisible()
    await expect(byTestId(page, 'kb-list-empty-create-button')).toBeVisible()
  })

  test('creates, renames, and deletes a knowledge base', async ({ page }) => {
    // create
    await byTestId(page, 'kb-list-create-button').click()
    await expect(byTestId(page, 'kb-form-drawer')).toBeVisible()
    await byTestId(page, 'kb-form-name-input').fill('Lab protocols')
    await byTestId(page, 'kb-form-submit-button').click()

    // a card appears with a status chip
    const card = page.locator('[data-testid^="kb-card-"]').first()
    await expect(card).toBeVisible()
    await expect(page.getByText('Lab protocols')).toBeVisible()

    // rename via the card edit button
    await page.locator('[data-testid^="kb-card-edit-button-"]').first().click()
    await expect(byTestId(page, 'kb-form-drawer')).toBeVisible()
    await byTestId(page, 'kb-form-name-input').fill('Renamed KB')
    await byTestId(page, 'kb-form-submit-button').click()
    await expect(page.getByText('Renamed KB')).toBeVisible()

    // delete via the card delete button + the AlertDialog confirm OK button
    // (testid `kb-card-delete-confirm-<id>-confirm`, distinct from the dialog root)
    await page.locator('[data-testid^="kb-card-delete-button-"]').first().click()
    await page
      .locator('[data-testid^="kb-card-delete-confirm-"][data-testid$="-confirm"]')
      .first()
      .click()
    await expect(byTestId(page, 'kb-list-empty')).toBeVisible()
  })
})

import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
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

  // FB-1 / TEST-50 (ITEM-28): the KB card must mirror the ProjectCard
  // precedent — a light-weight (font-normal → computed 400) title and NO
  // decorative leading icon — so the two list-grid entity cards read as one
  // design system. Guards against regressing to a bold Title heading + the
  // Library icon the initial build shipped.
  test('card title mirrors the project-card typography (no bold heading, no leading icon)', async ({ page }) => {
    await byTestId(page, 'kb-list-create-button').click()
    await expect(byTestId(page, 'kb-form-drawer')).toBeVisible()
    await byTestId(page, 'kb-form-name-input').fill('Lab protocols')
    await byTestId(page, 'kb-form-submit-button').click()

    const card = page.locator('[data-testid^="kb-card-"]').first()
    await expect(card).toBeVisible()

    const title = card.getByRole('heading', { name: 'Lab protocols' })
    // ProjectCard forces `!font-normal` (weight 400); a raw Title level={5}
    // would render bold (>=600). Assert the precedent weight.
    await expect(title).toHaveCSS('font-weight', '400')
    // The project card has no leading icon in its title header — neither
    // should the KB card (the removed <Library/>).
    await expect(title.locator('xpath=preceding-sibling::svg')).toHaveCount(0)
  })

  // FB-2 / TEST-51 (ITEM-28): the KB list must page like projects + chats —
  // "Showing N of M" + a Load More that reveals the next page — so a user with
  // many knowledge bases isn't handed an unbounded wall of cards.
  test('pages the knowledge base list with Load More', async ({ page, testInfra }) => {
    const token = await getAdminToken(testInfra.apiURL)
    // Seed 13 KBs (> the page size of 12) so a second page exists.
    for (let i = 0; i < 13; i++) {
      const res = await page.request.post(`${testInfra.apiURL}/api/knowledge-bases`, {
        headers: { Authorization: `Bearer ${token}` },
        data: { name: `Paging KB ${String(i).padStart(2, '0')}` },
      })
      expect(res.ok()).toBeTruthy()
    }
    await page.goto(`${testInfra.baseURL}/knowledge`)

    // First page shows 12 of 13 with a Load More button.
    await expect(page.getByText(/Showing 12 of 13 knowledge bases/)).toBeVisible({
      timeout: 30000,
    })
    const loadMore = page.getByRole('button', { name: 'Load More' })
    await expect(loadMore).toBeVisible()

    // Click → all 13 shown and the button disappears.
    await loadMore.click()
    await expect(page.getByText(/Showing 13 of 13 knowledge bases/)).toBeVisible({
      timeout: 15000,
    })
    await expect(page.getByRole('button', { name: 'Load More' })).toHaveCount(0)
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

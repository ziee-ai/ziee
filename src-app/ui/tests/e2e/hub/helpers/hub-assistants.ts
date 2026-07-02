import { Page, expect } from '@playwright/test'

/**
 * Create assistant from hub template.
 *
 * Clicking the primary "Use" button creates the assistant in one click
 * (no customize modal) and surfaces a success toast before navigating to
 * the assistants settings page. The `customize` arg is retained for caller
 * compatibility; the one-click flow does not surface an edit modal.
 */
export async function createAssistantFromHub(
  page: Page,
  assistantId: string,
  _customize?: {
    name?: string
    description?: string
    instructions?: string
  },
) {
  const assistantCard = page.getByTestId(`hub-assistant-card-${assistantId}`)
  // The card renders TWO "use" buttons: `hub-assistant-use-btn` (one-click)
  // and `hub-assistant-use-as-template-btn`. Anchor on the one-click testid.
  await assistantCard.getByTestId('hub-assistant-use-btn').click()

  // Success toast confirms the create round-tripped.
  await expect(
    page.locator('[data-sonner-toast][data-type="success"]').first(),
  ).toBeVisible({ timeout: 5000 })
}

/**
 * Get assistant card "Created" badge text (or null when absent).
 *
 * Allow several seconds — callers typically invoke this right after a
 * page.reload() and the hub store still needs to re-run its init →
 * loadAssistants → render cycle before the created_ids reach the DOM.
 */
export async function getAssistantCardStatus(
  page: Page,
  assistantId: string,
): Promise<string | null> {
  const badge = page.getByTestId(`hub-assistant-created-tag-${assistantId}`)

  const visible = await badge.isVisible({ timeout: 10000 }).catch(() => false)
  if (visible) {
    return await badge.textContent()
  }

  return null
}

/**
 * Check if assistant has "View" button (indicating it's been created).
 */
export async function isAssistantCreated(
  page: Page,
  assistantId: string,
): Promise<boolean> {
  // The View button's testid carries the assistant-name suffix
  // (`hub-assistant-view-btn-<name>`); getByTestId is an exact match, so the
  // bare id never resolved. assistantId IS that name suffix here.
  const viewButton = page
    .getByTestId(`hub-assistant-card-${assistantId}`)
    .getByTestId(`hub-assistant-view-btn-${assistantId}`)
  return await viewButton.isVisible({ timeout: 10000 }).catch(() => false)
}

/**
 * Get all assistant cards
 */
export async function getAssistantCards(page: Page) {
  // The card testid is `hub-assistant-card-<name>`, but the card ALSO contains
  // `hub-assistant-card-tag-<name>-<tag>` and `hub-assistant-card-dep-tag-...`
  // sub-elements that share the prefix. Exclude those so `.nth(i)` lands on a
  // real card, not a tag chip.
  return page.getByTestId(/^hub-assistant-card-(?!tag-|dep-tag-)/)
}

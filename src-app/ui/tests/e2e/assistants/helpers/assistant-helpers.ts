import { Page, Locator, expect } from '@playwright/test'
import { byTestId } from '../../testid'

/**
 * Navigation + interaction helpers for the assistants settings pages.
 *
 * Selectors are testid-first (i18n-safe). Row/action/tag testids are derived
 * from each row's `data-test-assistant-id` (e.g. `user-assistant-<uuid>`),
 * which the kit list rows + action buttons + tags all key off of.
 */

export async function goToUserAssistantsPage(page: Page, baseURL: string) {
  // The user's own assistants now live in settings (was the sidebar full-page
  // grid at /assistants). NOT `networkidle`: the realtime-sync SSE stream is a
  // persistent connection that keeps the network busy, so it may never settle —
  // wait on the card testid instead.
  await page.goto(`${baseURL}/settings/assistants`)
  await page.waitForLoadState('load')
  await byTestId(page, 'user-assistants-card').waitFor({ timeout: 15000 })
}

export async function goToTemplateAssistantsSettings(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/assistant-templates`)
  await page.waitForLoadState('load')
  await byTestId(page, 'template-assistants-card').waitFor({ timeout: 15000 })
}

/**
 * Assistant Form Drawer helpers
 */

export async function openCreateAssistantDrawer(page: Page, isUserPage = true) {
  await byTestId(
    page,
    isUserPage ? 'user-assistants-create-btn' : 'template-assistants-create-btn',
  ).click()
  // The form (inside the drawer) becoming visible is the drawer-open signal.
  await byTestId(page, 'assistant-form').waitFor({ state: 'visible' })
}

export async function fillAssistantForm(
  page: Page,
  data: {
    name: string
    description?: string
    instructions?: string
    parameters?: string
    enabled?: boolean
    isDefault?: boolean
  }
) {
  // Wait for the drawer form to be fully loaded by waiting for the name field.
  await byTestId(page, 'assistant-form-name').waitFor({ state: 'visible' })

  await byTestId(page, 'assistant-form-name').fill(data.name)

  if (data.description !== undefined) {
    await byTestId(page, 'assistant-form-description').fill(data.description)
  }

  if (data.instructions !== undefined) {
    await byTestId(page, 'assistant-form-instructions').fill(data.instructions)
  }

  if (data.parameters !== undefined) {
    await byTestId(page, 'assistant-form-parameters').fill(data.parameters)
  }

  if (data.enabled !== undefined) {
    await setAssistantSwitch(page, 'assistant-form-enabled', data.enabled)
  }

  if (data.isDefault !== undefined) {
    await setAssistantSwitch(page, 'assistant-form-default', data.isDefault)
  }
}

/**
 * Drive a Base UI Switch to a target checked state, robust to a lost click.
 * The assistant form's parameters textarea prettifies JSON on blur
 * (`form.setValue` → re-render); a switch click that lands mid-re-render is
 * dropped, so a single click can leave the controlled switch (and thus the RHF
 * field) unchanged. Click until `aria-checked` — which mirrors the controlled
 * `field.value` — reaches the target, so the value is committed before submit.
 */
export async function setAssistantSwitch(page: Page, testid: string, target: boolean) {
  const sw = byTestId(page, testid)
  await sw.waitFor({ state: 'visible', timeout: 5000 })
  for (let attempt = 0; attempt < 4; attempt++) {
    if ((await sw.getAttribute('aria-checked')) === String(target)) break
    await sw.click()
    await page.waitForTimeout(150)
  }
  await expect(sw).toHaveAttribute('aria-checked', String(target))
}

export async function submitAssistantForm(page: Page) {
  await byTestId(page, 'assistant-form-submit').click()
  // Don't wait for the drawer here — let the test verify the success message
  // first. The drawer closes automatically after a successful submission.
}

export async function cancelAssistantForm(page: Page) {
  await byTestId(page, 'assistant-form-cancel').click()

  // A dirty form prompts a "Discard unsaved changes?" confirm dialog before
  // closing; a pristine form closes immediately. Confirm the discard if shown.
  const discard = page.getByRole('alertdialog')
  try {
    await discard.waitFor({ state: 'visible', timeout: 1500 })
    // The confirm (Discard) action is the trailing button in the dialog footer.
    await discard.getByRole('button').last().click()
  } catch {
    // Pristine form — closed without a prompt.
  }

  await byTestId(page, 'assistant-form').waitFor({ state: 'hidden', timeout: 10000 })
}

/**
 * Row + action helpers (shared shape for user + template lists).
 *
 * Rows are keyed by `data-test-assistant-id="<prefix>-<uuid>"`. Action buttons
 * + tags derive their testids from that same value, so we read the attribute
 * off the matched row and target the buttons/tags by derived testid.
 */

export async function getUserAssistantRow(page: Page, assistantName: string): Promise<Locator> {
  return page.locator(
    `[data-test-assistant-id^="user-assistant-"]:has-text("${assistantName}")`,
  )
}

async function rowId(row: Locator): Promise<string> {
  const id = await row.getAttribute('data-test-assistant-id')
  if (!id) throw new Error('row is missing data-test-assistant-id')
  return id
}

export async function editUserAssistant(page: Page, assistantName: string) {
  const row = await getUserAssistantRow(page, assistantName)
  const id = await rowId(row)
  await byTestId(page, `${id}-edit`).click()
  // Wait for the edit drawer form to load.
  await byTestId(page, 'assistant-form').waitFor({ state: 'visible' })
  await byTestId(page, 'assistant-form-name').waitFor({ state: 'visible', timeout: 10000 })
}

export async function deleteUserAssistant(page: Page, assistantName: string) {
  const row = await getUserAssistantRow(page, assistantName)
  const id = await rowId(row)
  await byTestId(page, `${id}-delete`).click()

  // Confirm in the AlertDialog (Confirm kit). Its content testid is
  // `<id>-delete-confirm`; the confirm button is `<id>-delete-confirm-confirm`.
  await byTestId(page, `${id}-delete-confirm`).waitFor({ state: 'visible' })
  await byTestId(page, `${id}-delete-confirm-confirm`).click()
  await byTestId(page, `${id}-delete-confirm`).waitFor({ state: 'hidden' })
}

export async function getTemplateAssistantRow(page: Page, assistantName: string): Promise<Locator> {
  return page.locator(
    `[data-test-assistant-id^="template-assistant-"]:has-text("${assistantName}")`,
  )
}

export async function editTemplateAssistant(page: Page, assistantName: string) {
  const row = await getTemplateAssistantRow(page, assistantName)
  const id = await rowId(row)
  await byTestId(page, `${id}-edit`).click()
  await byTestId(page, 'assistant-form').waitFor({ state: 'visible' })
  await byTestId(page, 'assistant-form-name').waitFor({ state: 'visible', timeout: 10000 })
}

export async function deleteTemplateAssistant(page: Page, assistantName: string) {
  const row = await getTemplateAssistantRow(page, assistantName)
  const id = await rowId(row)
  await byTestId(page, `${id}-delete`).click()

  await byTestId(page, `${id}-delete-confirm`).waitFor({ state: 'visible' })
  await byTestId(page, `${id}-delete-confirm-confirm`).click()
  await byTestId(page, `${id}-delete-confirm`).waitFor({ state: 'hidden' })
}

/**
 * Pagination helpers (template page)
 */

export async function goToPage(page: Page, pageNumber: number) {
  const pagination = byTestId(page, 'template-assistants-pagination')
  // Numbered page links render the page number as their content.
  await pagination
    .locator('a', { hasText: new RegExp(`^${pageNumber}$`) })
    .click()
  await page.waitForLoadState('load')
}

export async function changePageSize(page: Page, size: number) {
  const trigger = byTestId(page, 'template-assistants-pagination-page-size')
  await trigger.click()
  await byTestId(page, `template-assistants-pagination-page-size-opt-${size}`).click()
  await page.waitForLoadState('load')
}

/**
 * Assertion helpers
 */

export async function assertUserAssistantExists(page: Page, assistantName: string, shouldExist = true) {
  const row = page.locator(`[data-test-assistant-id^="user-assistant-"]:has-text("${assistantName}")`)
  if (shouldExist) {
    await expect(row.first()).toBeVisible()
  } else {
    await expect(row).not.toBeVisible()
  }
}

export async function assertTemplateAssistantExists(page: Page, assistantName: string, shouldExist = true) {
  const row = page.locator(`[data-test-assistant-id^="template-assistant-"]:has-text("${assistantName}")`)
  if (shouldExist) {
    await expect(row.first()).toBeVisible()
  } else {
    await expect(row).not.toBeVisible()
  }
}

export async function assertUserAssistantHasTag(page: Page, assistantName: string, tagText: 'Default' | 'Inactive') {
  const row = await getUserAssistantRow(page, assistantName)
  const id = await rowId(row)
  const suffix = tagText === 'Default' ? 'default-tag' : 'inactive-tag'
  await expect(byTestId(page, `${id}-${suffix}`)).toBeVisible()
}

export async function assertEmptyState(page: Page, _message: string) {
  await expect(byTestId(page, 'user-assistants-empty')).toBeVisible()
}

export async function assertSuccessMessage(page: Page, message: string) {
  // Sonner toast carrying the success copy. `hasText` (not getByText) keeps the
  // gate clean; the message string is the operation-specific confirmation.
  await expect(
    page.locator('[data-sonner-toast]').filter({ hasText: message }).last(),
  ).toBeVisible({ timeout: 5000 })
}

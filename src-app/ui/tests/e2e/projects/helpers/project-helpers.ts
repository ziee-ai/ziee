import { Page, Locator, expect } from '@playwright/test'
import { byTestId } from '../../testid'

/**
 * Navigation + drawer helpers for the Projects E2E suite, rewritten onto
 * the kit's stable `data-testid`s (i18n-safe — visible text / role-names
 * change under translation, testids do not).
 */

// Don't wait for `networkidle` — when navigating AWAY from
// /projects/:id, the chat-input + MCP-modal mounts on the detail
// page can leave background activity (lazy store hydration,
// websocket reconnects) that delays networkidle past the playwright
// timeout. Waiting for the page's distinctive title testid is
// sufficient — by then the route component has mounted.
export async function goToProjectsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/projects`)
  await byTestId(page, 'project-list-title')
    .first()
    .waitFor({ timeout: 15000 })
}

export async function goToProjectDetail(
  page: Page,
  baseURL: string,
  projectId: string,
) {
  await page.goto(`${baseURL}/projects/${projectId}`)
  await page
    .locator('[data-test-project-title]')
    .first()
    .waitFor({ state: 'visible', timeout: 15000 })
}

export async function openCreateProjectDrawer(page: Page) {
  // The header "+" CTA (always present for users who can create) opens
  // the ProjectFormDrawer in create mode.
  await byTestId(page, 'project-list-create-button').click()
  await byTestId(page, 'project-form').waitFor({ state: 'visible' })
}

export async function fillProjectForm(
  page: Page,
  data: {
    name?: string
    description?: string
    instructions?: string
  },
) {
  await byTestId(page, 'project-form-name-input').waitFor({ state: 'visible' })
  if (data.name !== undefined) {
    await byTestId(page, 'project-form-name-input').fill(data.name)
  }
  if (data.description !== undefined) {
    await byTestId(page, 'project-form-description-textarea').fill(
      data.description,
    )
  }
  if (data.instructions !== undefined) {
    await byTestId(page, 'project-form-instructions-textarea').fill(
      data.instructions,
    )
  }
}

export async function submitProjectForm(page: Page) {
  await byTestId(page, 'project-form-submit-button').click()
}

export async function cancelProjectForm(page: Page) {
  await byTestId(page, 'project-form-cancel-button').click()
  await byTestId(page, 'project-form').waitFor({
    state: 'hidden',
    timeout: 10000,
  })
}

/**
 * Find a project card by its visible name. The card carries a stable
 * `data-test-project-name="<name>"` test hook (the testid itself is
 * keyed by the project id, which callers don't know — they identify a
 * project by the name they typed, which is dynamic data they created).
 */
export function getProjectCard(page: Page, projectName: string): Locator {
  return page.locator(`[data-test-project-name="${cssEsc(projectName)}"]`).first()
}

// Escape a value for use inside a CSS attribute selector string.
function cssEsc(s: string): string {
  return s.replace(/(["\\])/g, '\\$1')
}

/**
 * Click the inline Edit/Duplicate/Delete icon button on a project card.
 * The buttons are kit Buttons with testids `project-card-{edit,duplicate,
 * delete}-button-${id}`; scoping to the (single-project) card lets us
 * target them by testid PREFIX without needing the project id.
 *
 * For `Delete`, the click opens a Confirm (AlertDialog) — call
 * `confirmDeletePopconfirm` afterwards.
 */
export async function clickCardAction(
  page: Page,
  projectName: string,
  action: 'Edit' | 'Duplicate' | 'Delete',
) {
  const card = getProjectCard(page, projectName)
  const prefix = {
    Edit: 'project-card-edit-button-',
    Duplicate: 'project-card-duplicate-button-',
    Delete: 'project-card-delete-button-',
  }[action]
  await card.locator(`[data-testid^="${prefix}"]`).click()
}

/**
 * Confirm the open delete Confirm dialog. The kit Confirm renders its
 * primary (OK) button with a testid ending in `-confirm`; scope to the
 * open alertdialog so we hit the right one.
 */
export async function confirmDeletePopconfirm(page: Page) {
  await page
    .getByRole('alertdialog')
    .locator('[data-testid$="-confirm"]')
    .first()
    .click()
}

export async function assertProjectExists(
  page: Page,
  projectName: string,
  shouldExist = true,
) {
  if (shouldExist) {
    await expect(getProjectCard(page, projectName)).toBeVisible()
  } else {
    await expect(getProjectCard(page, projectName)).not.toBeVisible()
  }
}

export async function assertEmptyState(page: Page) {
  await expect(byTestId(page, 'project-list-empty')).toBeVisible()
}

/**
 * Assert a success toast appeared. Toast COPY is UI chrome (i18n-fragile)
 * so we assert on sonner's stable `data-type="success"` marker rather
 * than the text. The `_text` arg is kept for call-site readability.
 */
export async function assertSuccessMessage(
  page: Page,
  _text?: string | RegExp,
) {
  await expect(
    page.locator('[data-sonner-toast][data-type="success"]').first(),
  ).toBeVisible({ timeout: 10000 })
}

import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  goToUserAssistantsPage,
  openCreateAssistantDrawer,
  fillAssistantForm,
  submitAssistantForm,
  cancelAssistantForm,
  editUserAssistant,
  assertSuccessMessage,
} from './helpers/assistant-helpers'

/**
 * E2E — assistant edit-form *restoration* (audit r2-72c5d3b01cb5).
 *
 * `AssistantFormDrawer` restores the editing assistant's persisted values
 * into the form on every open via its `useEffect` → `form.setFieldsValue(...)`,
 * and `handleClose` calls `form.resetFields()`. The existing
 * `user-assistants.spec.ts::'should edit an existing assistant'` only asserts
 * the FIRST-open pre-fill. The genuinely uncovered behavior is the
 * discard-and-restore path: editing a field, cancelling (which never persists +
 * resets the form), then RE-opening the edit drawer must show the ORIGINAL
 * persisted values — the abandoned edits are gone, not leaked back in.
 */

test.describe('User Assistants — edit-form restoration', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToUserAssistantsPage(page, baseURL)
  })

  test('modifying then cancelling restores the original values on reopen', async ({
    page,
  }) => {
    const tag = Date.now().toString(36)
    const name = `Restore Assistant ${tag}`
    const originalDescription = 'Original description ' + tag
    const originalInstructions = 'You are the original assistant ' + tag

    // Create an assistant with known values.
    await openCreateAssistantDrawer(page)
    await fillAssistantForm(page, {
      name,
      description: originalDescription,
      instructions: originalInstructions,
    })
    await submitAssistantForm(page)
    await assertSuccessMessage(page, 'Assistant created successfully')

    // --- First edit open: form is restored with the persisted values. ---
    await editUserAssistant(page, name)
    await expect(byTestId(page, 'assistant-form-name')).toHaveValue(name)
    await expect(byTestId(page, 'assistant-form-description')).toHaveValue(
      originalDescription,
    )
    await expect(byTestId(page, 'assistant-form-instructions')).toHaveValue(
      originalInstructions,
    )

    // --- Modify several fields, then CANCEL (discard, no save). ---
    await byTestId(page, 'assistant-form-description').fill('ABANDONED edit ' + tag)
    await byTestId(page, 'assistant-form-instructions').fill('ABANDONED instructions')
    await cancelAssistantForm(page)

    // The row still shows the original name (nothing was persisted).
    await expect(
      page.locator(
        `[data-test-assistant-id^="user-assistant-"]:has-text("${name}")`,
      ),
    ).toBeVisible()

    // --- Reopen edit: the abandoned edits are gone; originals are restored. ---
    await editUserAssistant(page, name)
    await expect(byTestId(page, 'assistant-form-name')).toHaveValue(name)
    await expect(byTestId(page, 'assistant-form-description')).toHaveValue(
      originalDescription,
    )
    await expect(byTestId(page, 'assistant-form-instructions')).toHaveValue(
      originalInstructions,
    )
    // The abandoned values must NOT have leaked back into the form.
    await expect(byTestId(page, 'assistant-form-description')).not.toHaveValue(
      'ABANDONED edit ' + tag,
    )

    await cancelAssistantForm(page)
  })
})

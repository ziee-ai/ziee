import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — ImportSkillDialog (user-scope skill import): the drag-drop → validate
 * flow. Open the Import dialog from /skills, attach a SKILL.md to the Dragger,
 * click Validate, and assert the server-side validation result Alert renders.
 * (ImportSkillDialog.tsx — Dragger + /skills/validate + /skills/import.)
 */

test.describe('Skills — import dialog validate flow', () => {
  test('attaching a SKILL.md and validating surfaces the validation result', async ({
    page,
    testInfra,
  }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await page.goto(`${testInfra.baseURL}/settings/skills`)

    // Open the Import dialog.
    await byTestId(page, 'skill-list-import-button').click()
    const dialog = byTestId(page, 'skill-import-dialog')
    await expect(dialog).toBeVisible({ timeout: 15000 })

    // Attach a deliberately MALFORMED SKILL.md (no required frontmatter) to the
    // upload's hidden file input.
    await byTestId(dialog, 'skill-import-upload')
      .locator('input[type="file"]')
      .setInputFiles({
        name: 'SKILL.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('just a body with no frontmatter at all\n'),
      })

    // Validate → the server rejects it; the dialog shows the failure Alert
    // (tone="error" → role="alert").
    await byTestId(dialog, 'skill-import-validate-button').click()
    const alert = byTestId(dialog, 'skill-import-validation-alert')
    await expect(alert).toBeVisible({ timeout: 15000 })
    await expect(alert).toHaveAttribute('role', 'alert')
  })
})

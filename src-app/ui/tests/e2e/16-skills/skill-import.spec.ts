import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

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
    await page.goto(`${testInfra.baseURL}/skills`)

    // Open the Import dialog.
    await page.getByRole('button', { name: 'Import' }).click()
    const dialog = page.getByRole('dialog', { name: 'Import Skill' })
    await expect(dialog).toBeVisible({ timeout: 15000 })

    // Attach a deliberately MALFORMED SKILL.md (no required frontmatter) to the
    // Dragger's hidden file input.
    await dialog
      .locator('input[type="file"]')
      .setInputFiles({
        name: 'SKILL.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from('just a body with no frontmatter at all\n'),
      })

    // Validate → the server rejects it; the dialog shows the failure Alert.
    await dialog.getByRole('button', { name: 'Validate' }).click()
    await expect(dialog.getByText('Validation failed')).toBeVisible({
      timeout: 15000,
    })
  })
})

import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSkillsPage } from './helpers/skill-helpers'
import { byTestId } from '../testid.ts'

/**
 * E2E — the user-scope Import-Skill dialog (ImportSkillDialog.tsx).
 *
 * Audit gap: `list-page-renders.spec.ts` only renders the skills list; the
 * Import dialog (drop a SKILL.md → "Validate" → POST /api/skills/validate
 * → inline result Alert, then drag-drop/import) had no E2E. This opens the
 * dialog, drops a valid SKILL.md, runs Validate, and asserts the success
 * Alert from the real validate round-trip. Only the file upload is synthetic.
 */

const VALID_SKILL_MD = `---
name: e2e-import-skill
description: A throwaway skill used by the import-dialog E2E.
---

# E2E Import Skill

This is the body of a valid SKILL.md.
`

test.describe('Skills — user Import dialog validate', () => {
  test('drop a SKILL.md → Validate → success Alert', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToSkillsPage(page, baseURL)

    await byTestId(page, 'skill-list-import-button').click()

    const dialog = byTestId(page, 'skill-import-dialog')
    await expect(dialog).toBeVisible()

    await byTestId(dialog, 'skill-import-upload')
      .locator('input[type="file"]')
      .setInputFiles({
        name: 'SKILL.md',
        mimeType: 'text/markdown',
        buffer: Buffer.from(VALID_SKILL_MD, 'utf8'),
      })

    await byTestId(dialog, 'skill-import-validate-button').click()

    // tone="success" → the Alert renders role="status" (i18n-safe).
    const alert = byTestId(dialog, 'skill-import-validation-alert')
    await expect(alert).toBeVisible({ timeout: 30000 })
    await expect(alert).toHaveAttribute('role', 'status')
  })
})

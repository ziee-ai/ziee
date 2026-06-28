import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSkillsPage } from './helpers/skill-helpers'

/**
 * End-to-end skill flow (gap cae1aa1ca1ca): the built-in capability skills are
 * boot-synced (the "install" step), appear in the /skills list (the "list"
 * step), and opening one fetches + renders its SKILL.md body via
 * GET /api/skills/{id}/body (the "load_skill" content step — the same body the
 * skill_mcp load tool serves). Prior specs only covered admin gating + that the
 * page renders. (The chat-loop invocation needs a live model and is covered by
 * backend skill_mcp tests.)
 */
test.describe('Skills - install → list → load content', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
    await goToSkillsPage(page, testInfra.baseURL)
  })

  test('opening a built-in skill loads and renders its SKILL.md body', async ({
    page,
  }) => {
    // The skill rows are clickable cards (role="button") carrying a "Built-in"
    // badge; the Import button has no such badge, so this targets a skill row.
    const skillRow = page
      .locator('[role="button"]')
      .filter({ hasText: 'Built-in' })
      .first()
    await expect(skillRow).toBeVisible({ timeout: 15000 })
    await skillRow.click()

    // The detail drawer opens and fetches the on-disk SKILL.md body; once
    // loaded it renders under the "Skill content (SKILL.md)" heading.
    await expect(
      page.getByRole('heading', { name: /skill content \(SKILL\.md\)/i }),
    ).toBeVisible({ timeout: 15000 })
  })
})

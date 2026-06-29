import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSkillsPage } from './helpers/skill-helpers'
import { byTestId } from '../testid.ts'

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
    // The skill rows are clickable cards carrying a "Built-in" scope badge;
    // the Import button has no such badge, so this targets a skill row.
    const skillRow = page
      .locator('[data-testid^="skill-list-card-"]')
      .filter({ has: byTestId(page, 'skill-scope-badge-builtin') })
      .first()
    await expect(skillRow).toBeVisible({ timeout: 15000 })
    await skillRow.click()

    // The detail drawer opens and fetches the on-disk SKILL.md body; once
    // loaded it renders the body section (frontmatter-independent).
    await expect(byTestId(page, 'skill-detail-body')).toBeVisible({
      timeout: 15000,
    })
  })

  // audit id 0f5dfc6c8a9d — the detail drawer's metadata view (the Descriptions
  // table: Name / Files / Size, SkillDetailDrawer.tsx:44-255) was untested; the
  // test above only asserts the SKILL.md body heading.
  test('the skill detail drawer shows the metadata table', async ({ page }) => {
    const skillRow = page
      .locator('[data-testid^="skill-list-card-"]')
      .filter({ has: byTestId(page, 'skill-scope-badge-builtin') })
      .first()
    await expect(skillRow).toBeVisible({ timeout: 15000 })
    await skillRow.click()

    const drawer = byTestId(page, 'skill-detail-sheet-loaded')
    await expect(drawer).toBeVisible({ timeout: 15000 })
    // The Descriptions metadata table (Name / Files / Size rows) renders.
    await expect(byTestId(drawer, 'skill-detail-descriptions')).toBeVisible()
  })
})

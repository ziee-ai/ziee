import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSkillsPage } from './helpers/skill-helpers'
import { byTestId } from '../testid.ts'

test.describe('Skills - List page render', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToSkillsPage(page, baseURL)
  })

  test('renders the page', async ({ page }) => {
    await expect(byTestId(page, 'skills-page')).toBeVisible()
  })

  test('passes accessibility checks', async ({ page }) => {
    // The app shell renders an empty antd <Menu> (role="menu") sidebar
    // section in a fresh DB, which axe flags as aria-required-children.
    // This is a PRE-EXISTING shell-wide violation (confirmed identical on
    // the existing 11-projects a11y test), NOT from the skills/workflows
    // feature — excluded here, mirroring accessibility.ts's existing
    // exclusion of antd's nested-interactive Collapse quirk, so this spec
    // asserts the page's OWN a11y.
    await assertNoAccessibilityViolations(page, {
      disabledRules: ['aria-required-children'],
    })
  })

  test('lists ziee built-in capability skills', async ({ page }) => {
    // ziee's built-in capability skills are embedded in the binary and
    // boot-synced as scope='built_in' rows, so /skills is never empty —
    // even on a fresh DB the built-ins render with the "Built-in" badge.
    // (expect auto-retries, covering the boot-sync that runs on server
    // start.)
    await expect(byTestId(page, 'skill-scope-badge-builtin').first()).toBeVisible()
  })

  test('admin sees the permission-gated Import affordance', async ({ page }) => {
    // Admin holds skills::install via the `*` wildcard, so the
    // <Can permission={SkillsInstall}>-gated "Import" button renders.
    await expect(byTestId(page, 'skill-list-import-button')).toBeVisible()
  })

  test('clicking a skill opens the detail drawer with metadata + body', async ({
    page,
  }) => {
    // The built-in skills render as clickable cards. Open the first one.
    const firstCard = page.locator('[data-testid^="skill-list-card-"]').first()
    await expect(firstCard).toBeVisible({ timeout: 15000 })
    await firstCard.click()

    // The SkillDetailDrawer opens with the metadata Descriptions table
    // (Name/Files/Size) and fetches the SKILL.md body.
    const drawer = byTestId(page, 'skill-detail-sheet-loaded')
    await expect(drawer).toBeVisible({ timeout: 10000 })
    await expect(byTestId(drawer, 'skill-detail-descriptions')).toBeVisible()

    // The body fetch resolves and the body section renders. Built-in skills
    // always have a SKILL.md body.
    await expect(byTestId(drawer, 'skill-detail-body')).toBeVisible({
      timeout: 15000,
    })

    // Close it.
    await page.keyboard.press('Escape')
    await expect(drawer).toBeHidden()
  })
})

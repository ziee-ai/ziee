import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import { goToSkillsPage } from './helpers/skill-helpers'

test.describe('Skills - List page render', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToSkillsPage(page, baseURL)
  })

  test('renders the page heading', async ({ page }) => {
    await expect(
      page.getByRole('heading', { level: 4, name: 'Skills', exact: true }),
    ).toBeVisible()
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
    await expect(page.getByText('Built-in').first()).toBeVisible()
  })

  test('admin sees the permission-gated Import affordance', async ({ page }) => {
    // Admin holds skills::install via the `*` wildcard, so the
    // <Can permission={SkillsInstall}>-gated "Import" button renders.
    await expect(
      page.getByRole('button', { name: /import/i }),
    ).toBeVisible()
  })
})

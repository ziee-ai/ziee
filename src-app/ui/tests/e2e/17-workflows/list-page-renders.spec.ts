import { test, expect } from '../../fixtures/test-context'
import { assertNoAccessibilityViolations } from '../../utils/accessibility'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  assertWorkflowsEmptyState,
  goToWorkflowsPage,
} from './helpers/workflow-helpers'
import { byTestId } from '../testid'

test.describe('Workflows - List page render', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToWorkflowsPage(page, baseURL)
  })

  test('renders the page heading', async ({ page }) => {
    await expect(byTestId(page, 'wf-list-page-title')).toBeVisible()
  })

  test('passes accessibility checks', async ({ page }) => {
    // Exclude the pre-existing shell-wide empty-antd-Menu sidebar violation
    // (aria-required-children) — confirmed identical on the existing
    // 11-projects a11y test; not from this feature. Mirrors accessibility.ts's
    // existing nested-interactive exclusion so this spec asserts the page's
    // OWN a11y.
    await assertNoAccessibilityViolations(page, {
      disabledRules: ['aria-required-children'],
    })
  })

  test('shows empty state when no workflows are installed', async ({ page }) => {
    // A fresh test database has no installed workflows, so the antd
    // <Empty> ("No workflows installed yet — browse the Hub to install
    // one") renders. See WorkflowsList.tsx.
    await assertWorkflowsEmptyState(page)
  })

  test('admin sees the permission-gated Import affordance', async ({ page }) => {
    // Admin holds workflows::install via the `*` wildcard, so the
    // <Can permission={WorkflowsInstall}>-gated "Import" button renders.
    await expect(byTestId(page, 'wf-list-import-btn')).toBeVisible()
  })
})

import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'
import {
  fillProjectForm,
  getProjectCard,
  goToProjectsPage,
  openCreateProjectDrawer,
  submitProjectForm,
} from './helpers/project-helpers'

/**
 * Round-4 Option A layout — covers every visible element on the
 * project detail page so a future refactor can't silently lose one:
 *
 *   1. Header:    back, project name title, Edit, Duplicate
 *   2. Section 1: ChatInput (start a new conversation in this project)
 *   3. Section 2: Conversations (full-width list, NOT in a tab)
 *   4. Section 3: Project knowledge (inline preview + Manage drawer)
 *   5. Section 4: Instructions (preview + inline Edit)
 *   6. Section 5: About (only if project.description set)
 *   7. Section 6: Advanced (default-asset summary)
 *   8. Section 7: MCP defaults (header Edit button + body summary
 *      of approval mode + per-server auto-approved / disabled rules)
 *
 * Each section uses a stable `data-test-section="<id>"` selector so
 * spec assertions don't drift when copy or icons change.
 */
test.describe('Projects - detail page layout (Option A)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)

    // Seed a project with description + instructions so the
    // optional "About" + Instructions sections both render.
    await openCreateProjectDrawer(page)
    await fillProjectForm(page, {
      name: 'Layout Probe',
      description: 'Layout sanity-check project.',
      instructions: 'Speak only in haiku.',
    })
    await submitProjectForm(page)

    // Land on the detail page. Wait for at least one section to be
    // rendered before returning — the detail page shows <Spin /> until
    // Stores.ProjectDetail.project finishes loading, and any test that
    // queries [data-test-section=…] right after navigation would see
    // an empty DOM.
    await getProjectCard(page, 'Layout Probe').click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
    await page
      .locator('[data-test-project-title="Layout Probe"]')
      .waitFor({ state: 'visible', timeout: 15000 })
  })

  test('header has Back, project title, Edit, Duplicate; no "New chat" button', async ({
    page,
  }) => {
    await expect(byTestId(page, 'project-detail-back-button')).toBeVisible()
    await expect(page.locator('[data-test-project-title="Layout Probe"]')).toBeVisible()
    await expect(byTestId(page, 'project-detail-edit-button')).toBeVisible()
    await expect(
      byTestId(page, 'project-detail-duplicate-button'),
    ).toBeVisible()
    // "New chat" button is gone — replaced by the inline ChatInput.
    await expect(
      byTestId(page, 'project-detail-new-chat-button'),
    ).toHaveCount(0)
  })

  test('renders every section in the expected vertical order', async ({
    page,
  }) => {
    // The last section, `mcp-defaults`, is an extension-contributed panel
    // rendered via a `lazy()` + <Suspense> boundary (mcp/project-extension), so
    // it mounts a tick after the page's own sections. Wait for it before
    // snapshotting, otherwise the DOM order is read mid-Suspense and the trailing
    // section is missing (the panel itself is correct — see the `:182`/`:198`
    // card tests; this is purely a lazy-mount race in this snapshot assertion).
    await expect(
      page.locator('[data-test-section="mcp-defaults"]'),
    ).toBeVisible()

    // Sections in DOM order. Playwright's `nth-of-type` doesn't work
    // for sibling sections so we read all and assert the list.
    const sectionIds = await page
      .locator('[data-test-section]')
      .evaluateAll(els => els.map(el => el.getAttribute('data-test-section')))

    // ChatInput → Conversations → ProjectMeta (Description +
    // Instructions + Knowledge subsections) → Advanced → extension-
    // contributed advanced_settings cards (currently: MCP defaults via
    // mcp/project-extension). ProjectMeta wraps the description/
    // instructions/knowledge group in a single card so its inner
    // sections render between project-meta and advanced.
    expect(sectionIds).toEqual([
      'chat-input',
      'conversations',
      'project-meta',
      'description',
      'instructions',
      'knowledge',
      'advanced',
      'mcp-defaults',
    ])
  })

  test('ChatInput section renders the composer', async ({ page }) => {
    const section = page.locator('[data-test-section="chat-input"]')
    await expect(section).toBeVisible()
    await expect(
      section.locator('textarea[placeholder*="Type your message"]'),
    ).toBeVisible({ timeout: 10000 })
  })

  test('Conversations section is full-width and not behind a tab', async ({
    page,
  }) => {
    const section = page.locator('[data-test-section="conversations"]')
    await expect(section).toBeVisible()
    // Empty state on a freshly-created project (no chats yet).
    await expect(byTestId(section, 'project-conversations-empty')).toBeVisible()
    // No tab role anywhere on this page (Option A dropped Tabs).
    await expect(page.getByRole('tab')).toHaveCount(0)
  })

  test('Knowledge section shows inline empty-state chip + Manage button', async ({
    page,
  }) => {
    const section = page.locator('[data-test-section="knowledge"]')
    await expect(section).toBeVisible()
    await expect(
      section.locator('[data-test-files-empty="true"]'),
    ).toBeVisible()
    await expect(
      byTestId(section, 'project-knowledge-manage-button'),
    ).toBeVisible()
  })

  test('Manage button opens the full file management drawer', async ({
    page,
  }) => {
    await byTestId(
      page.locator('[data-test-section="knowledge"]'),
      'project-knowledge-manage-button',
    ).click()
    const drawer = page.getByRole('dialog')
    await expect(drawer).toBeVisible()
    // Drawer body renders the ProjectFilesManagePanel — its empty-state
    // marker confirms the knowledge file panel rendered.
    await expect(byTestId(drawer, 'file-project-empty')).toBeVisible()
  })

  test('Instructions section shows the project instruction text', async ({
    page,
  }) => {
    const section = page.locator('[data-test-section="instructions"]')
    await expect(section).toBeVisible()
    // The textContent of the Paragraph carries the full instructions.
    await expect(
      section.locator('[data-test-instructions="Speak only in haiku."]'),
    ).toBeVisible()
  })

  test('Description ("About") section renders only when description is set', async ({
    page,
  }) => {
    const section = page.locator('[data-test-section="description"]')
    await expect(section).toBeVisible()
    // The description text is dynamic data this test created; assert it
    // via the stable `data-test-description` hook (i18n-safe).
    await expect(
      section.locator('[data-test-description="Layout sanity-check project."]'),
    ).toBeVisible()
  })

  test('Advanced section summarises default assistant + model status', async ({
    page,
  }) => {
    const advanced = page.locator('[data-test-section="advanced"]')
    await expect(advanced).toBeVisible()
    // Defaults not set → both should report "false".
    await expect(
      advanced.locator('[data-test-default-assistant-set="false"]'),
    ).toBeVisible()
    await expect(
      advanced.locator('[data-test-default-model-set="false"]'),
    ).toBeVisible()
  })

  test('MCP Defaults card shows approval mode + header Edit button', async ({
    page,
  }) => {
    // MCP defaults moved to their own section after the project↔mcp
    // inversion — the mcp module contributes a panel via the
    // `advanced_settings` slot, rendered as `data-test-section="mcp-defaults"`.
    const mcp = page.locator('[data-test-section="mcp-defaults"]')
    await expect(mcp).toBeVisible()
    // Default MCP approval mode on a fresh project = manual_approve.
    await expect(
      mcp.locator('[data-test-mcp-approval-mode="manual_approve"]'),
    ).toBeVisible()
    // Edit button lives in the Card `extra` slot in the header.
    await expect(byTestId(mcp, 'mcp-project-edit-btn')).toBeVisible()
  })

  test('"Edit" header button opens the shared MCP modal in project scope', async ({
    page,
  }) => {
    await byTestId(
      page.locator('[data-test-section="mcp-defaults"]'),
      'mcp-project-edit-btn',
    ).click()

    // The shared McpConfigModal opens (project scope is the only way to
    // reach it from this button).
    await expect(byTestId(page, 'mcp-config-modal')).toBeVisible({
      timeout: 5000,
    })

    // "Save as Default" should be hidden in project scope (it writes
    // user_mcp_defaults — orthogonal).
    await expect(
      byTestId(page, 'mcp-config-save-default-btn'),
    ).toHaveCount(0)
  })
})

/**
 * Edge case: a project with NO description should skip rendering the
 * "About" section entirely (no empty placeholder). Keeps the layout
 * tight for users who didn't fill description in.
 */
test.describe('Projects - detail page layout (no description)', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)

    await openCreateProjectDrawer(page)
    // Intentionally omit description.
    await fillProjectForm(page, { name: 'No About' })
    await submitProjectForm(page)
    await getProjectCard(page, 'No About').click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
  })

  test('description section is omitted when project.description is unset', async ({
    page,
  }) => {
    await expect(
      page.locator('[data-test-section="description"]'),
    ).toHaveCount(0)
    // Other sections still present.
    await expect(page.locator('[data-test-section="chat-input"]')).toBeVisible()
    await expect(page.locator('[data-test-section="conversations"]')).toBeVisible()
    await expect(page.locator('[data-test-section="knowledge"]')).toBeVisible()
    await expect(page.locator('[data-test-section="instructions"]')).toBeVisible()
    await expect(page.locator('[data-test-section="advanced"]')).toBeVisible()
  })

  test('Instructions empty-state copy appears when no instructions set', async ({
    page,
  }) => {
    const instructions = page.locator('[data-test-section="instructions"]')
    await expect(instructions).toBeVisible()
    // Empty branch: no instruction paragraph renders (the
    // `data-test-instructions` hook only exists when instructions set).
    await expect(
      instructions.locator('[data-test-instructions]'),
    ).toHaveCount(0)
  })
})

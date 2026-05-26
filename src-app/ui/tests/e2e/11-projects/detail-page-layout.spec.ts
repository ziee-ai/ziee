import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  fillProjectForm,
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
 *   7. Section 6: Advanced (default-asset summary + Configure MCP button)
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
    await page.locator('.ant-card', { hasText: 'Layout Probe' }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)
    await page
      .locator('[data-test-project-title="Layout Probe"]')
      .waitFor({ state: 'visible', timeout: 15000 })
  })

  test('header has Back, project title, Edit, Duplicate; no "New chat" button', async ({
    page,
  }) => {
    await expect(
      page.getByRole('button', { name: 'Back to projects' }),
    ).toBeVisible()
    await expect(page.locator('[data-test-project-title="Layout Probe"]')).toBeVisible()
    // antd icons contribute their name to the button's accessible
    // name (Edit = "edit Edit", Duplicate = "copy Duplicate"). Use
    // non-anchored regex.
    await expect(page.getByRole('button', { name: /edit/i }).first()).toBeVisible()
    await expect(
      page.getByRole('button', { name: /duplicate/i }).first(),
    ).toBeVisible()
    // "New chat" button is gone — replaced by the inline ChatInput.
    await expect(page.getByRole('button', { name: /^new chat$/i })).toHaveCount(0)
  })

  test('renders every section in the expected vertical order', async ({
    page,
  }) => {
    // Sections in DOM order. Playwright's `nth-of-type` doesn't work
    // for sibling sections so we read all and assert the list.
    const sectionIds = await page
      .locator('[data-test-section]')
      .evaluateAll(els => els.map(el => el.getAttribute('data-test-section')))

    // ChatInput → Conversations → Knowledge → Instructions →
    // (Description only when set) → Advanced.
    expect(sectionIds).toEqual([
      'chat-input',
      'conversations',
      'knowledge',
      'instructions',
      'description',
      'advanced',
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
    await expect(
      section.getByText(/no conversations in this project yet/i),
    ).toBeVisible()
    // No antd tab role anywhere on this page (Option A dropped Tabs).
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
      section.getByRole('button', { name: /manage knowledge files/i }),
    ).toBeVisible()
  })

  test('Manage button opens the full file management drawer', async ({
    page,
  }) => {
    await page
      .locator('[data-test-section="knowledge"]')
      .getByRole('button', { name: /manage knowledge files/i })
      .click()
    const drawer = page.locator('.ant-drawer.ant-drawer-open')
    await expect(drawer).toBeVisible()
    // Drawer body renders the ProjectFilesPanel which uses
    // "Knowledge files" as its header (empty-state branch).
    await expect(drawer.getByText(/knowledge files/i).first()).toBeVisible()
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
    await expect(section.getByText('Layout sanity-check project.')).toBeVisible()
  })

  test('Advanced section summarises defaults + has Configure MCP defaults button', async ({
    page,
  }) => {
    const section = page.locator('[data-test-section="advanced"]')
    await expect(section).toBeVisible()
    // Defaults not set → both should report "false".
    await expect(
      section.locator('[data-test-default-assistant-set="false"]'),
    ).toBeVisible()
    await expect(
      section.locator('[data-test-default-model-set="false"]'),
    ).toBeVisible()
    // Default MCP approval mode on a fresh project = manual_approve.
    await expect(
      section.locator('[data-test-mcp-approval-mode="manual_approve"]'),
    ).toBeVisible()
    // Configure MCP defaults button (admin → has ProjectsEdit).
    await expect(
      section.getByRole('button', { name: /configure mcp defaults/i }),
    ).toBeVisible()
  })

  test('"Configure MCP defaults" opens the shared MCP modal in project scope', async ({
    page,
  }) => {
    await page
      .locator('[data-test-section="advanced"]')
      .getByRole('button', { name: /configure mcp defaults/i })
      .click()

    // The shared McpConfigModal opens; in project scope its title
    // switches to "MCP Defaults for Project". Match on the title
    // text directly — the page can host multiple ant-modal-content
    // portals (one from this page's explicit <McpConfigModal />, one
    // from <McpMenuItem> inside the embedded <ChatInput>'s + menu)
    // which trips strict-mode on the bare .ant-modal-content selector.
    await expect(
      page.getByText('MCP Defaults for Project'),
    ).toBeVisible({ timeout: 5000 })

    // "Save as Default" should be hidden in project scope (it
    // writes user_mcp_defaults — orthogonal). Asserting it's not on
    // the page is enough — the close button + the rest of the modal
    // shell are antd-built.
    await expect(
      page.getByRole('button', { name: /save as default/i }),
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
    await page.locator('.ant-card', { hasText: 'No About' }).click()
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
    await expect(
      page
        .locator('[data-test-section="instructions"]')
        .getByText(/no instructions yet/i),
    ).toBeVisible()
  })
})

import { Page, expect } from '@playwright/test'

/**
 * Navigation + drawer helpers for the Projects E2E suite. Mirrors the
 * shape of `06-assistants/helpers/assistant-helpers.ts` so reviewers
 * familiar with assistants tests can read these at a glance.
 */

export async function goToProjectsPage(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/projects`)
  await page.waitForLoadState('networkidle')
  await page
    .getByRole('heading', { level: 4, name: /projects/i })
    .first()
    .waitFor({ timeout: 10000 })
}

export async function goToProjectDetail(
  page: Page,
  baseURL: string,
  projectId: string,
) {
  await page.goto(`${baseURL}/projects/${projectId}`)
  await page.waitForLoadState('networkidle')
}

export async function openCreateProjectDrawer(page: Page) {
  await page
    .getByRole('button', { name: /create project|new project|^plus$/i })
    .first()
    .click()
  await page.locator('.ant-drawer.ant-drawer-open').waitFor({ state: 'visible' })
}

export async function fillProjectForm(
  page: Page,
  data: {
    name?: string
    description?: string
    instructions?: string
  },
) {
  await page.getByLabel('Name').waitFor({ state: 'visible' })
  if (data.name !== undefined) {
    await page.getByLabel('Name').fill(data.name)
  }
  if (data.description !== undefined) {
    await page.getByLabel('Description').fill(data.description)
  }
  if (data.instructions !== undefined) {
    await page.getByLabel('Instructions').fill(data.instructions)
  }
}

export async function submitProjectForm(page: Page) {
  // Per `[[project_ui_e2e_drawer_selectors]]`: scope by
  // `.ant-btn-primary[type="submit"]` rather than the text-match.
  // Text matching is fragile across button-label changes (Create/Save
  // tense, icon contribution to accessible name, loading state hiding
  // the label) and the CSS-selector is the canonical drawer submit.
  await page
    .locator('.ant-drawer.ant-drawer-open .ant-btn-primary[type="submit"]')
    .click()
}

export async function cancelProjectForm(page: Page) {
  await page
    .locator('.ant-drawer.ant-drawer-open')
    .getByRole('button', { name: /^cancel$/i })
    .click()
  await page
    .locator('.ant-drawer.ant-drawer-open')
    .waitFor({ state: 'hidden', timeout: 10000 })
}

/**
 * Find a project card by its visible name. Returns the card locator.
 * Cards are antd cards whose title contains the project name; we use
 * `locator('...').filter({ hasText: name })` which is stable across
 * the icon + dropdown changes.
 */
export function getProjectCard(page: Page, projectName: string) {
  return page
    .locator('.ant-card')
    .filter({ hasText: projectName })
    .first()
}

function escapeRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
}

/**
 * Click the inline Edit/Duplicate/Delete icon button on a project
 * card. The round-3 ProjectCard rewrite replaced the Dropdown
 * "Project actions" menu with three inline icon buttons whose
 * aria-labels are "Edit {name}" / "Duplicate {name}" / "Delete {name}".
 *
 * For `Delete`, the click opens an antd Popconfirm — call
 * `confirmDeletePopconfirm` afterwards.
 */
export async function clickCardAction(
  page: Page,
  projectName: string,
  action: 'Edit' | 'Duplicate' | 'Delete',
) {
  const card = getProjectCard(page, projectName)
  await card
    .getByRole('button', {
      name: new RegExp(`^${action} ${escapeRe(projectName)}$`, 'i'),
    })
    .click()
}

/**
 * Click the primary action of the antd Popconfirm currently open on
 * the page. Selector is stable across okText changes per
 * `[[project_ui_e2e_drawer_selectors]]`.
 */
export async function confirmDeletePopconfirm(page: Page) {
  await page
    .locator('.ant-popconfirm .ant-btn-primary')
    .first()
    .click()
}

/** @deprecated Round-3 removed the Dropdown menu — use `clickCardAction` instead. */
export async function openProjectCardMenu(_page: Page, projectName: string) {
  throw new Error(
    `openProjectCardMenu is gone — the Dropdown menu was replaced by inline ` +
      `icon buttons in round 3. Update the test to call clickCardAction(page, '${projectName}', 'Edit'|'Duplicate'|'Delete') ` +
      `(and confirmDeletePopconfirm for the delete case).`,
  )
}

/** @deprecated See `openProjectCardMenu`. */
export async function clickCardMenuItem(
  _page: Page,
  _itemName: 'Edit' | 'Duplicate' | 'Delete',
) {
  throw new Error('clickCardMenuItem is gone — use clickCardAction.')
}

export async function assertProjectExists(
  page: Page,
  projectName: string,
  shouldExist = true,
) {
  if (shouldExist) {
    await expect(getProjectCard(page, projectName)).toBeVisible()
  } else {
    await expect(getProjectCard(page, projectName)).not.toBeVisible()
  }
}

export async function assertEmptyState(page: Page) {
  // "No projects yet" appears both as the sidebar widget's <Text>
  // empty-state AND the main page's <Title> empty-state. Scope to
  // the main page's <h3> heading so the strict mode check doesn't
  // match the sidebar instance.
  await expect(
    page.getByRole('heading', { name: /no projects yet/i }),
  ).toBeVisible()
}

export async function assertSuccessMessage(page: Page, text: string | RegExp) {
  await expect(page.locator('.ant-message')).toContainText(text)
}

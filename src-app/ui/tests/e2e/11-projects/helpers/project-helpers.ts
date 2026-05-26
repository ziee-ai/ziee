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
  await page
    .locator('.ant-drawer.ant-drawer-open')
    .getByRole('button', { name: /^create$|^save$/i })
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

export async function openProjectCardMenu(page: Page, projectName: string) {
  const card = getProjectCard(page, projectName)
  await card.getByRole('button', { name: /project actions/i }).click()
}

export async function clickCardMenuItem(
  page: Page,
  itemName: 'Edit' | 'Duplicate' | 'Delete',
) {
  await page.getByRole('menuitem', { name: itemName }).click()
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
  await expect(page.getByText(/no projects yet/i)).toBeVisible()
}

export async function assertSuccessMessage(page: Page, text: string | RegExp) {
  await expect(page.locator('.ant-message')).toContainText(text)
}

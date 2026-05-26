import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import {
  fillProjectForm,
  goToProjectsPage,
  openCreateProjectDrawer,
  submitProjectForm,
} from './helpers/project-helpers'

test.describe('Projects - New conversation in project', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)
    await goToProjectsPage(page, baseURL)

    await openCreateProjectDrawer(page)
    await fillProjectForm(page, {
      name: 'Project With Chat',
      instructions: 'Pretend to be a pirate.',
    })
    await submitProjectForm(page)
  })

  test('clicking New chat from detail page navigates to /chat with project_id', async ({
    page,
  }) => {
    await page.locator('.ant-card', { hasText: 'Project With Chat' }).click()
    await page.waitForURL(/\/projects\/[0-9a-f-]+$/)

    const newChatBtn = page.getByRole('button', { name: /new chat/i })
    await expect(newChatBtn).toBeVisible()
    await newChatBtn.click()

    // The detail page wires "New chat" to /chat?project_id=…
    await page.waitForURL(/\/chat\?project_id=[0-9a-f-]+/)
  })
})

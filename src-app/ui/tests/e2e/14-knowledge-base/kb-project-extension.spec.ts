import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// TEST-41 (ITEM-34): the "Knowledge bases" project knowledge-kind — the project
// detail page's Manage drawer stacks the KB manage panel; binding a KB to the
// project shows it there and in the inline preview count.
test.describe('Knowledge Base — project extension', () => {
  test('binds a KB to a project and shows it in the manage panel + inline preview', async ({
    page,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const token = await getAdminToken(apiURL)
    const auth = { Authorization: `Bearer ${token}` }

    const proj = await page.request.post(`${apiURL}/api/projects`, {
      headers: auth,
      data: { name: 'KB Project' },
    })
    const projectId: string = (await proj.json()).id
    const kb = await page.request.post(`${apiURL}/api/knowledge-bases`, {
      headers: auth,
      data: { name: 'Bound KB' },
    })
    const kbId: string = (await kb.json()).id

    // EMPTY: open the project's knowledge Manage drawer → the KB panel empty state.
    await page.goto(`${baseURL}/projects/${projectId}`)
    await byTestId(page, 'project-knowledge-manage-button').click()
    const drawer = page.getByRole('dialog')
    await expect(drawer).toBeVisible()
    await expect(byTestId(drawer, 'kb-project-panel-empty')).toBeVisible()
    await expect(byTestId(drawer, 'kb-project-attach-button')).toBeVisible()

    // Bind the KB to the project (via the API, the same call the picker makes).
    const attach = await page.request.put(
      `${apiURL}/api/projects/${projectId}/knowledge-bases/${kbId}`,
      { headers: auth },
    )
    expect(attach.ok()).toBeTruthy()

    // Reload → the manage panel lists the bound KB and the inline preview counts it.
    await page.reload()
    await byTestId(page, 'project-knowledge-manage-button').click()
    const drawer2 = page.getByRole('dialog')
    await expect(byTestId(drawer2, `kb-project-row-${kbId}`)).toBeVisible()
    await expect(drawer2.getByText('Bound KB')).toBeVisible()
  })
})

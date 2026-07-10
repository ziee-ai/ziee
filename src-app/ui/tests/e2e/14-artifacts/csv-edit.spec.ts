import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import { seedProjectFile } from '../file/helpers'

async function headText(page: import('@playwright/test').Page, baseURL: string, fileId: string) {
  const token = await getAdminToken(baseURL)
  return page.evaluate(
    async ([base, id, t]) => {
      const r = await fetch(`${base}/api/files/${id}/text`, {
        headers: { Authorization: `Bearer ${t}` },
      })
      return r.ok ? await r.text() : ''
    },
    [baseURL, fileId, token] as const,
  )
}

// TEST-20: a CSV deliverable opens in the editable grid; editing a cell + adding
// a row, Save, and the exported CSV reflects the change losslessly.
test.describe('Artifacts — CSV grid canvas edit', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('csv opens in grid, edit cell + add row, save + export reflects it', async ({
    page,
    testInfra,
  }) => {
    const fileId = await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Csv ${Date.now()}`,
      filename: 'data.csv',
      content: 'name,score\nAlice,10\n',
      mime: 'text/csv',
    })

    await page.goto(`${testInfra.baseURL}/files/${fileId}`)
    await page.getByTestId('canvas-edit-toggle').click()
    await expect(page.getByTestId('canvas-csv-grid')).toBeVisible()

    // Edit the first data cell.
    const cell = page.getByTestId('csv-cell-0-0')
    await cell.fill('Bob')
    await expect(cell).toHaveValue('Bob')

    // Add a row, fill it.
    await page.getByTestId('csv-add-row').click()
    await page.getByTestId('csv-cell-1-0').fill('Carol')
    await page.getByTestId('csv-cell-1-1').fill('20')
    await expect(page.getByTestId('csv-cell-1-0')).toHaveValue('Carol')

    await expect(page.getByTestId('canvas-save')).toBeEnabled()
    await page.getByTestId('canvas-save').click()
    await expect(page.getByTestId('file-version-bar')).toBeVisible()

    // Authoritative persistence: the saved head CSV carries the edited cell +
    // the added row (polled to absorb the async Save→append round-trip).
    await expect
      .poll(() => headText(page, testInfra.baseURL, fileId), { timeout: 15000 })
      .toContain('Bob')
    expect(await headText(page, testInfra.baseURL, fileId)).toContain('Carol')
  })
})

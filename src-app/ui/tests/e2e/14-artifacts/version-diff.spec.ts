import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile } from '../file/helpers'

// TEST-28: after ≥2 versions exist, selecting an older version + Compare opens the
// diff dialog showing added/removed lines (append-only history is inspectable).
test.describe('Artifacts — version diff', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('edit creates v2; selecting v1 + Compare shows the diff dialog', async ({
    page,
    testInfra,
  }) => {
    const fileId = await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Diff ${Date.now()}`,
      filename: 'notes.md',
      content: 'Original line one\n',
      mime: 'text/markdown',
    })

    await page.goto(`${testInfra.baseURL}/files/${fileId}`)
    await page.getByTestId('canvas-edit-toggle').click()
    const editable = page.getByTestId('canvas-edit-body').locator('[contenteditable="true"]').first()
    await editable.click()
    await page.keyboard.press('End')
    await page.keyboard.press('Enter')
    await editable.pressSequentially('Added second line')
    await page.getByTestId('canvas-save').click()

    // Two versions now exist → the bar appears. Select v1 (the non-head version).
    await expect(page.getByTestId('file-version-bar')).toBeVisible()
    await page.getByTestId('file-version-select').click()
    await page.getByRole('option', { name: /^v1/ }).click()

    // Viewing an old version reveals Compare.
    await expect(page.getByTestId('file-version-old-tag')).toBeVisible()
    await page.getByTestId('file-version-compare').click()

    // The diff dialog renders the two-version comparison.
    await expect(page.getByTestId('file-version-compare-dialog')).toBeVisible()
    await expect(page.getByText('Added second line', { exact: false }).first()).toBeVisible()
  })
})

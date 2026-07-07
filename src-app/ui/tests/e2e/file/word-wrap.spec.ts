import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { seedProjectFile, openPreviewDrawer } from './helpers'

// TEST-7 (ITEM-6, ITEM-8): word-wrap toggle on a file with a very long line.
const LONG_LINE = 'x'.repeat(4000)
const CONTENT = `short line\n${LONG_LINE}\nanother short line\n`

test.describe('File viewer — word wrap', () => {
  test.beforeEach(async ({ page, testInfra }) => {
    await loginAsAdmin(page, testInfra.baseURL)
  })

  test('toggling wrap removes horizontal overflow', async ({ page, testInfra }) => {
    await seedProjectFile(page, testInfra.baseURL, {
      projectName: `Wrap ${Date.now()}`,
      filename: 'wrap.txt',
      content: CONTENT,
      mime: 'text/plain',
    })
    const drawer = await openPreviewDrawer(page, 'wrap.txt')
    const raw = drawer.getByTestId('raw-code-view')
    await raw.waitFor({ state: 'visible' })

    // Default: wrap OFF, long line overflows horizontally.
    await expect(raw).toHaveAttribute('data-word-wrap', 'off')
    const overflowsBefore = await raw.evaluate(el => {
      const pre = el.querySelector('pre.shiki') as HTMLElement | null
      return pre ? pre.scrollWidth > el.clientWidth + 4 : false
    })
    expect(overflowsBefore).toBe(true)

    // Toggle wrap ON.
    await drawer.getByTestId('file-viewer-wrap-btn').click()
    await expect(raw).toHaveAttribute('data-word-wrap', 'on')
    // The long line now wraps — the pre no longer exceeds the container width.
    await expect
      .poll(async () =>
        raw.evaluate(el => {
          const pre = el.querySelector('pre.shiki') as HTMLElement | null
          return pre ? pre.scrollWidth <= el.clientWidth + 4 : true
        }),
      )
      .toBe(true)

    // Toggle back OFF restores horizontal overflow.
    await drawer.getByTestId('file-viewer-wrap-btn').click()
    await expect(raw).toHaveAttribute('data-word-wrap', 'off')
  })
})

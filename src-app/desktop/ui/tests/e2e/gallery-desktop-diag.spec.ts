import { test } from '@playwright/test'

test('DIAG: what does the gallery render + console on settings-about', async ({ page }) => {
  const logs: string[] = []
  page.on('console', m => logs.push(`[${m.type()}] ${m.text()}`))
  page.on('pageerror', e => logs.push(`[PAGEERROR] ${String(e)}`))
  await page.goto('/gallery.html?surface=settings-about&state=loaded&theme=light', {
    waitUntil: 'domcontentloaded',
  })
  await page.waitForTimeout(15000)
  const hasRoot = await page.locator('[data-testid="gallery-root"]').count()
  const bodyLen = (await page.locator('body').innerHTML()).length
  const bodyStart = (await page.locator('body').innerHTML()).slice(0, 400)
  console.log('=== gallery-root count:', hasRoot, 'bodyLen:', bodyLen)
  console.log('=== body start:', bodyStart)
  console.log('=== console/errors:\n' + logs.slice(0, 30).join('\n'))
})

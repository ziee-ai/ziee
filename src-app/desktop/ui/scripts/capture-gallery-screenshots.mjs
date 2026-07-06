/**
 * Capture per-page light+dark screenshots of the seeded gallery for a
 * vision-review pass. Dev-only tooling; output is git-ignored (env-specific).
 *
 * Usage: node scripts/capture-gallery-screenshots.mjs [--out=DIR] [--url=URL]
 */
import { chromium } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'

const outArg = process.argv.find(a => a.startsWith('--out='))
const urlArg = process.argv.find(a => a.startsWith('--url='))
const OUT = outArg ? outArg.slice(6) : '/tmp/gallery-shots'
const BASE = urlArg ? urlArg.slice(6) : 'http://localhost:1466/dev-gallery.html'
const THEMES = ['light', 'dark']

const browser = await chromium.launch()
const summary = {}
for (const theme of THEMES) {
  const dir = path.join(OUT, theme)
  fs.mkdirSync(dir, { recursive: true })
  const page = await browser.newPage({ viewport: { width: 1280, height: 1000 } })
  await page.goto(`${BASE}?theme=${theme}`, { waitUntil: 'networkidle' })
  await page.waitForTimeout(2500)
  const sections = await page.locator('[data-testid^="gallery-page-"]').all()
  let n = 0
  for (const s of sections) {
    const id = (await s.getAttribute('data-testid')).replace('gallery-page-', '')
    try {
      await s.scrollIntoViewIfNeeded()
      await s.screenshot({ path: path.join(dir, `${id}.png`) })
      n++
    } catch (e) {
      console.log(`  ! ${theme}/${id}: ${e.message.slice(0, 60)}`)
    }
  }
  // Also a full-page montage.
  await page.screenshot({ path: path.join(OUT, `_full-${theme}.png`), fullPage: true })
  summary[theme] = n
  await page.close()
}
await browser.close()
console.log('captured:', JSON.stringify(summary), '→', OUT)

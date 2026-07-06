/**
 * Multi-state screenshots + empty/error bug collector.
 *
 * For every rendered page, captures loaded / empty / error (the data-state set)
 * per theme by driving the single-combo URL (?surface=&state=&theme=), and
 * records any console error / ErrorBoundary crash per (surface, state). Empty +
 * error are where most bugs hide — this is the finding pass.
 *
 * Screenshot id: surface__state__theme.png
 * Usage: node scripts/capture-gallery-states.mjs [--out=DIR] [--url=BASE] [--states=loaded,empty,error]
 */
import { chromium } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'

const arg = (n, d) => (process.argv.find(a => a.startsWith(`--${n}=`)) || `--${n}=${d}`).split('=').slice(1).join('=')
const OUT = arg('out', '/tmp/gallery-states')
const BASE = arg('url', 'http://localhost:1466/dev-gallery.html')
const STATES = arg('states', 'loaded,empty,error').split(',')
const THEMES = ['light', 'dark']

const browser = await chromium.launch()

// 1. Enumerate page slugs from the default browse view.
const enumPage = await browser.newPage({ viewport: { width: 1280, height: 900 } })
await enumPage.goto(BASE, { waitUntil: 'networkidle' })
await enumPage.waitForTimeout(2000)
const slugs = []
for (const s of await enumPage.locator('[data-testid^="gallery-page-"]').all()) {
  slugs.push((await s.getAttribute('data-testid')).replace('gallery-page-', ''))
}
await enumPage.close()

// 2. Capture each (slug, state, theme) + collect findings.
const findings = []
let shots = 0
for (const slug of slugs) {
  for (const state of STATES) {
    for (const theme of THEMES) {
      const p = await browser.newPage({ viewport: { width: 1280, height: 900 } })
      const url = `${BASE}?surface=${slug}&state=${state}&theme=${theme}`
      try {
        await p.goto(url, { waitUntil: 'networkidle' })
        await p.waitForTimeout(state === 'error' ? 1500 : 1200)
        const sec = p.locator(`[data-testid="gallery-page-${slug}"]`)
        const dir = path.join(OUT, theme)
        fs.mkdirSync(dir, { recursive: true })
        await sec.screenshot({ path: path.join(dir, `${slug}__${state}.png`) })
        shots++
        // Only count a REAL ErrorBoundary render: after the DOM has settled the
        // per-surface boundary's fallback (`data-testid="gallery-crash"`) is
        // still present. This is deliberately NOT keyed off `console.error` — a
        // store logging a failed fetch in error-mode is expected, not a crash;
        // only a boundary that CAUGHT a render throw and is still showing its
        // fallback at settle is a genuine crash.
        if (theme === 'light') {
          const crash = sec.locator('[data-testid="gallery-crash"]')
          if (await crash.count()) {
            const label = await crash.first().getAttribute('data-crash-label')
            findings.push({ slug, state, crashes: ['CRASH: ' + (label || slug)] })
          }
        }
      } catch (e) {
        findings.push({ slug, state, crashes: ['NAV: ' + e.message.slice(0, 80)] })
      }
      await p.close()
    }
  }
}
await browser.close()

console.log(`captured ${shots} state screenshots → ${OUT}`)
console.log(`\n=== crashes in empty/error/loaded states (${findings.length}) ===`)
for (const f of findings) console.log(`  ${f.slug} [${f.state}]: ${f.crashes.join(' | ')}`)
fs.writeFileSync(path.join(OUT, 'findings.json'), JSON.stringify(findings, null, 2))

/**
 * Form filled/invalid state capture. For the form-bearing overlays (create/edit
 * drawers), drives Playwright on the OPEN overlay to produce:
 *   - filled  : every visible text input filled with a sample value;
 *   - invalid : required fields cleared + submit clicked → inline validation.
 * These states are interaction-produced (not static renders), so they live in
 * the capture layer, not the mock. Screenshots: <slug>__<state>.png.
 *
 * Usage: node scripts/capture-gallery-forms.mjs [--out=DIR] [--url=BASE]
 */
import { chromium } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'

const arg = (n, d) => (process.argv.find(a => a.startsWith(`--${n}=`)) || `--${n}=${d}`).split('=').slice(1).join('=')
const OUT = arg('out', '/tmp/gallery-forms')
const BASE = arg('url', 'http://localhost:1466/dev-gallery.html')

// Form overlays worth filled/invalid coverage (a subset of the wired overlays).
const FORM_OVERLAYS = [
  'overlay-create-user-drawer',
  'overlay-llm-provider-drawer',
  'overlay-edit-user-drawer',
  'overlay-assistant-form-drawer',
  'overlay-llm-repository-drawer',
]

const browser = await chromium.launch()
fs.mkdirSync(OUT, { recursive: true })
const findings = []

for (const slug of FORM_OVERLAYS) {
  // FILLED
  {
    const p = await browser.newPage({ viewport: { width: 1280, height: 900 } })
    await p.goto(`${BASE}?surface=${slug}&state=open`, { waitUntil: 'networkidle' })
    await p.waitForTimeout(1200)
    const inputs = p.locator('[role="dialog"] input[type="text"], [role="dialog"] input:not([type]), [role="dialog"] textarea')
    const n = await inputs.count()
    for (let i = 0; i < n; i++) {
      try {
        await inputs.nth(i).fill(`Sample value ${i + 1}`, { timeout: 800 })
      } catch {}
    }
    await p.waitForTimeout(300)
    await p.screenshot({ path: path.join(OUT, `${slug}__filled.png`) })
    await p.close()
  }
  // INVALID — clear required inputs + click the primary submit.
  {
    const p = await browser.newPage({ viewport: { width: 1280, height: 900 } })
    const errs = []
    p.on('pageerror', e => errs.push(e.message.slice(0, 80)))
    await p.goto(`${BASE}?surface=${slug}&state=open`, { waitUntil: 'networkidle' })
    await p.waitForTimeout(1200)
    const req = p.locator('[role="dialog"] input[required], [role="dialog"] input[aria-required="true"]')
    const rn = await req.count()
    for (let i = 0; i < rn; i++) {
      try { await req.nth(i).fill('', { timeout: 800 }) } catch {}
    }
    const submit = p.locator('[role="dialog"] button', { hasText: /save|create|add|submit/i }).last()
    try { await submit.click({ timeout: 1000 }) } catch {}
    await p.waitForTimeout(500)
    await p.screenshot({ path: path.join(OUT, `${slug}__invalid.png`) })
    if (errs.length) findings.push({ slug, state: 'invalid', errs: [...new Set(errs)] })
    await p.close()
  }
}
await browser.close()
console.log(`captured filled/invalid for ${FORM_OVERLAYS.length} form overlays → ${OUT}`)
if (findings.length) {
  console.log('crashes during form interaction:')
  for (const f of findings) console.log(`  ${f.slug} [${f.state}]: ${f.errs.join(' | ')}`)
} else {
  console.log('no crashes during fill/submit-invalid')
}

/**
 * CROP-REVIEW MANIFEST GENERATOR — Layer 2 of the UI-defect detection system.
 *
 * The deterministic geometry audit (Layer 1) can't see everything: role
 * differentiation, semantic order, affordance presence, disproportion of intent.
 * Those are the taxonomy's `[V]` (vision) classes, and per process-rule 4 they
 * must be reviewed on COMPONENT CROPS at native resolution (section testids), not
 * full-page screenshots. This script:
 *
 *   1. Enumerates every gallery surface × the review viewports.
 *   2. Screenshots each SECTION crop (card / section / region / panel testid) at
 *      deviceScaleFactor 2 (native res), plus a SCROLLED-MIDDLE crop for every
 *      scroll container (taxonomy K4).
 *   3. Emits `CROP_REVIEW_MANIFEST.md` — the review checklist whose rubric is the
 *      set of `[V]` taxonomy lines (parsed live from docs/DEFECT_TAXONOMY.md, so
 *      the rubric can never drift from the taxonomy) PLUS the mandatory
 *      ABSENCE-questions (process-rule 3) and the per-surface named acceptance
 *      call-outs (C1 badge-before-key for citations, C7/C12/C13 role & decoration
 *      for chat, J5 tab density, C11 unlabeled-icon meaning).
 *
 * Crops go to `src/dev/gallery/crops/` (git-ignored — regenerated on demand); the
 * MANIFEST markdown is the committed, PR-able artifact.
 *
 *   node scripts/gen-crop-review-manifests.mjs [--url=BASE] [--no-shots]
 */
import { chromium } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const UI_DIR = path.resolve(__dirname, '..')
const GALLERY_DIR = path.resolve(UI_DIR, 'src/dev/gallery')
const CROPS_DIR = path.join(GALLERY_DIR, 'crops')
const TAXONOMY = path.resolve(UI_DIR, 'docs/DEFECT_TAXONOMY.md')

const arg = (n, d) =>
  (process.argv.find(a => a.startsWith(`--${n}=`)) || `--${n}=${d}`).split('=').slice(1).join('=')
const flag = n => process.argv.includes(`--${n}`)
const PORT = process.env.GALLERY_PORT || '1420'
const BASE = arg('url', `http://localhost:${PORT}/dev-gallery.html`)
const NO_SHOTS = flag('no-shots')

const VIEWPORTS = [
  { name: 'mobile', width: 390, height: 844 },
  { name: 'desktop', width: 1280, height: 900 },
]

// Per-surface named acceptance call-outs (the human-caught misses that MUST be
// asked explicitly when their crop is reviewed).
const NAMED_CALLOUTS = {
  'settings-citations': [
    'C1 (acceptance #4): is any status tag/badge (e.g. "verified") ordered BEFORE the citation key it qualifies? A badge must FOLLOW its label — "vaswani2017attention (verified)", never "(verified) vaswani2017attention".',
  ],
  'deep-chat-long': [
    'C7 (acceptance #6): can you tell a USER message from an ASSISTANT message at a glance? If they share background/alignment/decoration, the roles are indistinguishable.',
    'C12/C13 (acceptance #13): the user-message avatar — is it a bare gray placeholder circle with no image/initials? What would be LOST if it were removed? (An avatar that conveys nothing is valueless decoration.)',
    'K4: review the SCROLLED-MIDDLE crop too — does conversation context ("In project: …", title, mode) remain visible after scrolling, or does it scroll away?',
  ],
  'deep-chat-tool-approval': [
    'C9/C10 (acceptance #7): the "Tool Approval Required" block — is the icon on its OWN line (split from its label)? Is the icon oversized (>1.6×) relative to the text it labels?',
  ],
  'deep-chat-right-panel-file': [
    'J5 (acceptance #9): the right-panel tab strip — boxed/button-look tabs in a narrow side panel where a quiet UNDERLINE variant belongs?',
    'J6 (acceptance #10): the file-viewer action group — do peer icon-only buttons mix variants (Download=outline vs open-sidebar/open-new-tab=ghost)?',
    'C11 (acceptance #10): the open-in-new-tab button — does its icon communicate "open in new tab" (ExternalLink), or an ambiguous icon?',
  ],
  'deep-chat-right-panel-literature': [
    'J5: same tab-strip density question as the file panel.',
  ],
}

// The mandatory ABSENCE questions (process-rule 3) — asked on EVERY crop.
const ABSENCE_QUESTIONS = [
  'What differentiation is MISSING? (roles/kinds that should look different but don\'t — C7/C8)',
  'What affordance is MISSING? (focus/hover/loading/disabled/error state, empty-state CTA — G1-G6/H1)',
  'What state is MISSING or stuck? (loading that never clears, error without retry — H2/H3)',
  'What decoration is VALUELESS? For each decorative element: what would be lost if removed? (C13)',
  'For a SCROLLABLE crop: what chrome/affordance leaves the viewport when scrolled? (K1/K4)',
]

function parseVisionRubric() {
  if (!fs.existsSync(TAXONOMY)) return { byCat: {}, header: '' }
  const lines = fs.readFileSync(TAXONOMY, 'utf8').split('\n')
  const byCat = {}
  let cat = 'misc'
  for (const l of lines) {
    const cm = l.match(/^##\s+([A-Z])\.\s+(.*)/)
    if (cm) { cat = `${cm[1]}. ${cm[2]}`; continue }
    // a [V]-bearing bullet (vision rubric line) — also include [V/T] and [L/G]-with-V
    const bm = l.match(/^-\s+([A-Z]\d+)\s+\[([^\]]+)\]\s+(.*)/)
    if (bm && /V/.test(bm[2])) {
      ;(byCat[cat] ??= []).push(`${bm[1]} — ${bm[3].replace(/\*/g, '').trim()}`)
    }
  }
  return { byCat }
}

async function main() {
  const { byCat } = parseVisionRubric()
  fs.mkdirSync(CROPS_DIR, { recursive: true })

  let surfaces = { pages: [], deep: [], seeded: [] }
  const shots = [] // { surface, viewport, section, file, scrolled }

  if (!NO_SHOTS) {
    const browser = await chromium.launch({ args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu'] })
    const en = await browser.newPage({ viewport: { width: 1280, height: 900 } })
    await en.goto(BASE, { waitUntil: 'networkidle' })
    await en.waitForTimeout(2500)
    const pages = []
    for (const s of await en.locator('[data-testid^="gallery-page-"]').all())
      pages.push((await s.getAttribute('data-testid')).replace('gallery-page-', ''))
    const deep = await en.evaluate(() => window.__GALLERY_DEEP_STATES__ || [])
    const seeded = await en.evaluate(() => window.__GALLERY_SEEDED__ || [])
    await en.close()
    surfaces = { pages: pages.filter(p => ![...deep, ...seeded].includes(p)), deep, seeded }

    // Only crop the priority surfaces (named call-outs) exhaustively; sample the
    // rest — the manifest's value is the rubric + the acceptance crops, not a
    // thousand PNGs.
    const priority = new Set([
      ...Object.keys(NAMED_CALLOUTS),
      ...surfaces.pages.filter(p => /assistant|provider|citation|hardware|memory|user/.test(p)),
    ])
    const toCrop = [...priority].filter(s =>
      [...surfaces.pages, ...surfaces.deep, ...surfaces.seeded].includes(s),
    )

    for (const surface of toCrop) {
      for (const vp of VIEWPORTS) {
        const p = await browser.newPage({ viewport: vp, deviceScaleFactor: 2 })
        try {
          await p.goto(`${BASE}?surface=${surface}&state=loaded&theme=light`, { waitUntil: 'domcontentloaded', timeout: 20000 })
          await p.waitForTimeout(900)
          // section-level crop targets
          const targets = await p.evaluate(() => {
            const sel = '[data-slot="card"],[data-testid$="-card"],[data-testid*="section"],[role="region"],[data-testid$="-panel"]'
            const out = []
            for (const el of document.querySelectorAll(sel)) {
              const r = el.getBoundingClientRect()
              const tid = el.getAttribute('data-testid') || el.getAttribute('data-slot') || 'section'
              if (r.width > 40 && r.height > 24) out.push({ tid, x: r.x, y: r.y, w: r.width, h: r.height })
            }
            return out.slice(0, 12)
          })
          let idx = 0
          for (const t of targets) {
            const safe = `${surface}__${vp.name}__${(t.tid || 'sec').replace(/[^a-z0-9_-]/gi, '_')}__${idx++}`.slice(0, 120)
            const file = path.join(CROPS_DIR, `${safe}.png`)
            try {
              await p.screenshot({ path: file, clip: { x: Math.max(0, t.x), y: Math.max(0, t.y), width: Math.min(t.w, vp.width), height: Math.min(t.h, 1600) } })
              shots.push({ surface, viewport: vp.name, section: t.tid, file: path.relative(GALLERY_DIR, file), scrolled: false })
            } catch { /* clip out of range */ }
          }
          // K4: scrolled-middle crop for the primary scroll container
          const scrolled = await p.evaluate(() => {
            const sc = [...document.querySelectorAll('*')].find(el => {
              const s = getComputedStyle(el)
              return (s.overflowY === 'auto' || s.overflowY === 'scroll') && el.scrollHeight > el.clientHeight + 200
            })
            if (!sc) return null
            sc.scrollTop = Math.round((sc.scrollHeight - sc.clientHeight) / 2)
            const r = sc.getBoundingClientRect()
            return { x: r.x, y: r.y, w: r.width, h: r.height }
          })
          if (scrolled) {
            await p.waitForTimeout(200)
            const file = path.join(CROPS_DIR, `${surface}__${vp.name}__SCROLLED-MIDDLE.png`)
            try {
              await p.screenshot({ path: file, clip: { x: Math.max(0, scrolled.x), y: Math.max(0, scrolled.y), width: Math.min(scrolled.w, vp.width), height: Math.min(scrolled.h, 1400) } })
              shots.push({ surface, viewport: vp.name, section: 'SCROLLED-MIDDLE (K4)', file: path.relative(GALLERY_DIR, file), scrolled: true })
            } catch { /* */ }
          }
        } catch { /* nav */ }
        await p.close()
      }
    }
    await browser.close()
  }

  // ── write the manifest ────────────────────────────────────────────────────
  const md = []
  md.push('# Crop-review manifest (GENERATED — Layer 2)\n')
  md.push(
    '> `node scripts/gen-crop-review-manifests.mjs`. Vision review happens on **component crops at native resolution** (process-rule 4). Each crop under `crops/` is reviewed against the rubric below. The rubric IS the taxonomy\'s `[V]` (vision) classes, parsed live from `docs/DEFECT_TAXONOMY.md` so it can never drift. **Process-rule 3: answer the ABSENCE questions, not only "what looks wrong".**\n',
  )

  md.push('## The vision rubric (every `[V]` taxonomy class)\n')
  for (const [cat, lines] of Object.entries(byCat)) {
    md.push(`### ${cat}`)
    for (const l of lines) md.push(`- [ ] ${l}`)
    md.push('')
  }

  md.push('## ABSENCE questions — ask on EVERY crop (process-rule 3)\n')
  for (const q of ABSENCE_QUESTIONS) md.push(`- [ ] ${q}`)
  md.push('')

  md.push('## Per-surface acceptance call-outs\n')
  for (const [surface, callouts] of Object.entries(NAMED_CALLOUTS)) {
    md.push(`### \`${surface}\``)
    for (const c of callouts) md.push(`- [ ] ${c}`)
    md.push('')
  }

  md.push('## Captured crops\n')
  if (NO_SHOTS) md.push('_(run without `--no-shots` to capture PNGs under `crops/`)_\n')
  else {
    md.push(`${shots.length} crop(s) captured under \`crops/\`.\n`)
    const bySurface = {}
    for (const s of shots) (bySurface[s.surface] ??= []).push(s)
    for (const [surface, list] of Object.entries(bySurface).sort()) {
      md.push(`### \`${surface}\``)
      for (const s of list) md.push(`- [${s.viewport}] ${s.section}${s.scrolled ? ' 🔽' : ''} — \`crops/${path.basename(s.file)}\``)
      md.push('')
    }
  }

  fs.writeFileSync(path.join(GALLERY_DIR, 'CROP_REVIEW_MANIFEST.md'), md.join('\n'))
  // keep the PNGs out of git
  fs.writeFileSync(path.join(CROPS_DIR, '.gitignore'), '*.png\n')
  console.log(`crop-review: ${shots.length} crops, manifest → src/dev/gallery/CROP_REVIEW_MANIFEST.md`)
}

main().catch(e => { console.error(e); process.exit(2) })

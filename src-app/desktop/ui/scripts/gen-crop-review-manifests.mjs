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
import crypto from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { enumerateSurfaces } from './lib/gallery-surfaces.mjs'

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

// Section-level crop targets — card / section / region / panel testids.
const SECTION_SELECTOR =
  '[data-slot="card"],[data-testid$="-card"],[data-testid*="section"],[role="region"],[data-testid$="-panel"]'

// Deep/seeded surfaces run an async mount-time store seed + a lazy page (the real
// ConversationPage settling from the mock-API cassette). Wait for the section
// testid, then poll the section's rendered size/markup until it stops changing —
// so the crop captures REAL content, not the Suspense fallback or half-laid-out
// cards. Pages settle synchronously and only need a short beat.
async function waitForSurfaceReady(p, surface, cls) {
  const sec = p.locator(`[data-testid="gallery-page-${surface}"]`)
  await sec.waitFor({ state: 'visible', timeout: 15000 }).catch(() => {})
  if (cls === 'deep' || cls === 'seeded') {
    await waitForStable(p, `[data-testid="gallery-page-${surface}"]`, { quietMs: 700, timeout: 14000 })
  } else {
    await p.waitForTimeout(600)
  }
}

// Poll a scope element's (childElementCount, scrollHeight, innerHTML length) until
// it holds steady for `quietMs`, bounded by `timeout`. A cheap DOM-idle settle.
async function waitForStable(p, selector, { quietMs = 600, timeout = 12000, tick = 150 } = {}) {
  const deadline = Date.now() + timeout
  let last = ''
  let stableSince = 0
  while (Date.now() < deadline) {
    const sig = await p.evaluate(sel => {
      const el = document.querySelector(sel)
      if (!el) return ''
      return `${el.childElementCount}:${el.scrollHeight}:${el.querySelectorAll('*').length}`
    }, selector)
    if (sig && sig === last) {
      if (!stableSince) stableSince = Date.now()
      if (Date.now() - stableSince >= quietMs) return
    } else {
      last = sig
      stableSince = 0
    }
    await p.waitForTimeout(tick)
  }
}

// Capture ONE element crop. Uses the element handle's own screenshot so Playwright
// SCROLLS THE TARGET INTO VIEW (even inside a nested chat scroller that has
// auto-scrolled to the bottom) and clips exactly to the element's box. This is the
// fix for the deep-surface duplication bug: the old code computed a viewport-clip
// with `y: Math.max(0, rect.y)`, so any card with a NEGATIVE viewport y (scrolled
// out of the inner message list) clamped to y=0 and captured the gallery header
// chrome — producing byte-identical stub crops. Returns true on a real capture.
async function captureHandle(handle, file, maxH = 1600) {
  await handle.scrollIntoViewIfNeeded({ timeout: 2500 }).catch(() => {})
  const box = await handle.boundingBox()
  if (!box || box.width < 40 || box.height < 24) return false // not visible / collapsed
  // OCCLUSION GUARD: on narrow viewports a right-panel drawer covers the message
  // list, so a tool card is in the layout (boundingBox non-null) but painted OVER.
  // Capturing it would grab the covering panel — the same class of wrong-region bug
  // as the old clip clamp. Require the element to be the topmost paint at its own
  // upper-center point (and inside the viewport) before capturing it.
  const onTop = await handle.evaluate(el => {
    const r = el.getBoundingClientRect()
    const cx = r.left + r.width / 2
    const cy = r.top + Math.min(r.height / 2, 24)
    if (cx < 0 || cy < 0 || cx > window.innerWidth || cy > window.innerHeight) return false
    const top = document.elementFromPoint(cx, cy)
    return !!top && (el === top || el.contains(top))
  })
  if (!onTop) return false
  if (box.height <= maxH) {
    await handle.screenshot({ path: file })
  } else {
    // Very tall element: capture just the top `maxH` band (fresh, on-screen coords).
    const page = await handle.ownerFrame().then(f => f.page())
    await page.screenshot({ path: file, clip: { x: box.x, y: box.y, width: box.width, height: maxH } })
  }
  return true
}

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
  const shots = [] // { surface, viewport, section, file, scrolled, cls }
  // sha256(crop bytes) → [ {surface, section, file} ] — the non-duplication gate.
  const hashes = new Map()

  if (!NO_SHOTS) {
    const browser = await chromium.launch({ args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu'] })

    // Enumerate EVERY surface class from the single source (lib/gallery-surfaces),
    // so deep/seeded conversation states are never silently skipped.
    const en = await browser.newPage({ viewport: { width: 1280, height: 900 } })
    const classes = await enumerateSurfaces(en, BASE)
    await en.close()
    surfaces = { pages: classes.pages, deep: classes.deep, seeded: classes.seeded }
    const clsOf = new Map([
      ...surfaces.pages.map(s => [s, 'page']),
      ...surfaces.deep.map(s => [s, 'deep']),
      ...surfaces.seeded.map(s => [s, 'seeded']),
    ])

    // Only crop the priority surfaces (named call-outs) exhaustively; sample the
    // rest — the manifest's value is the rubric + the acceptance crops, not a
    // thousand PNGs.
    const priority = new Set([
      ...Object.keys(NAMED_CALLOUTS),
      ...surfaces.pages.filter(p => /assistant|provider|citation|hardware|memory|user/.test(p)),
    ])
    const toCrop = [...priority].filter(s => clsOf.has(s))

    for (const surface of toCrop) {
      const cls = clsOf.get(surface)
      for (const vp of VIEWPORTS) {
        const p = await browser.newPage({ viewport: vp, deviceScaleFactor: 2 })
        try {
          // Deep/seeded surfaces seed their own transient state and IGNORE `&state=`;
          // forcing `state=loaded` on them is meaningless. Only data-state pages honor it.
          const params = new URLSearchParams({ surface, theme: 'light' })
          if (cls === 'page') params.set('state', 'loaded')
          await p.goto(`${BASE}?${params}`, { waitUntil: 'domcontentloaded', timeout: 20000 })
          // Wait for the SPECIFIC section testid + a class-aware content settle
          // BEFORE crop, so deep/seeded surfaces show real components.
          await waitForSurfaceReady(p, surface, cls)

          // section-level crop targets, SCOPED to the surface section (never the
          // browse chrome) — enumerate handles so each can be scrolled into view.
          const sec = p.locator(`[data-testid="gallery-page-${surface}"]`)
          const rawHandles = await sec.locator(SECTION_SELECTOR).elementHandles()
          // Read each handle's tid, then PRIORITIZE container-level targets
          // (panels / regions / *section*) ahead of individual cards, so the 12-cap
          // never drops the surface-distinguishing container (e.g. `chat-right-panel`,
          // whose tab strip + file-viewer actions the right-panel callouts review)
          // in favor of the message-list cards that repeat across surfaces.
          const tagged = []
          for (const h of rawHandles) {
            const tid = (await h.evaluate(
              el => el.getAttribute('data-testid') || el.getAttribute('data-slot') || 'section',
            )) || 'section'
            const isContainer = /-panel$|section|region/i.test(tid)
            tagged.push({ h, tid, prio: isContainer ? 0 : 1 })
          }
          tagged.sort((a, b) => a.prio - b.prio) // stable within group (Node sort is stable)
          // Cap generously (20) so the container-first sort never starves the
          // per-variant tool cards a deep conversation renders (~13) — they are the
          // whole point of the deep-chat crops. Still bounded to avoid a thousand PNGs.
          let idx = 0
          for (const { h, tid } of tagged.slice(0, 20)) {
            const safe = `${surface}__${vp.name}__${tid.replace(/[^a-z0-9_-]/gi, '_')}__${idx++}`.slice(0, 120)
            const file = path.join(CROPS_DIR, `${safe}.png`)
            try {
              if (await captureHandle(h, file)) {
                const hash = crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex')
                ;(hashes.get(hash) ?? hashes.set(hash, []).get(hash)).push({ surface, section: tid, viewport: vp.name, file: path.basename(file) })
                shots.push({ surface, viewport: vp.name, section: tid, file: path.relative(GALLERY_DIR, file), scrolled: false, cls })
              }
            } catch { /* not capturable */ }
          }

          // K4: scrolled-middle crop for the surface's primary scroll container.
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
            await p.waitForTimeout(250)
            const file = path.join(CROPS_DIR, `${surface}__${vp.name}__SCROLLED-MIDDLE.png`)
            try {
              await p.screenshot({ path: file, clip: { x: Math.max(0, scrolled.x), y: Math.max(0, scrolled.y), width: Math.min(scrolled.w, vp.width), height: Math.min(scrolled.h, 1400) } })
              const hash = crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex')
              ;(hashes.get(hash) ?? hashes.set(hash, []).get(hash)).push({ surface, section: 'SCROLLED-MIDDLE (K4)', viewport: vp.name, file: path.basename(file) })
              shots.push({ surface, viewport: vp.name, section: 'SCROLLED-MIDDLE (K4)', file: path.relative(GALLERY_DIR, file), scrolled: true, cls })
            } catch { /* */ }
          }
        } catch { /* nav */ }
        await p.close()
      }
    }
    await browser.close()
  }

  // ── NON-DUPLICATION GATE ───────────────────────────────────────────────────
  // A crop whose filename promises component TYPE X must not be byte-identical to a
  // crop promising a DIFFERENT component TYPE Y — that only happens when the
  // generator captured a shared fallback/chrome region instead of the real
  // components (the deep-surface stub-crop bug). We compare NORMALIZED base types
  // (trailing instance ids stripped) so legitimately-identical SIBLING instances of
  // ONE component — 4 identical GPU cards, two collapsed same-name tool headers —
  // never trip the gate; only distinct component TYPES sharing bytes do.
  const baseType = tid =>
    tid.replace(/-(?:\d+|toolu_[a-z0-9_]+)$/i, '') // hardware-gpu-info-card-0 / mcp-tooluse-card-toolu_web_1
  const collisions = []
  for (const [hash, group] of hashes) {
    const distinctTypes = new Set(group.map(g => baseType(g.section)))
    if (group.length > 1 && distinctTypes.size > 1) {
      collisions.push({ hash: hash.slice(0, 12), members: group })
    }
  }
  if (collisions.length && !NO_SHOTS) {
    console.error(`\n✗ CROP DUPLICATION GATE FAILED: ${collisions.length} hash(es) shared by DIFFERENT components:`)
    for (const c of collisions) {
      console.error(`  [${c.hash}] ${c.members.length} identical crops of different sections:`)
      for (const m of c.members) console.error(`      ${m.surface} · ${m.section} (${m.viewport}) → ${m.file}`)
    }
    console.error('\nThis means the generator captured a fallback/chrome region instead of the real component.')
    process.exit(3)
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

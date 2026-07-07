/**
 * GALLERY GEOMETRY AUDIT — the deterministic (CANNOT-MISS) layout-defect pass.
 *
 * Layer 1 of the UI-defect detection system (see
 * `src-app/ui/docs/DEFECT_TAXONOMY.md`). It loads EVERY gallery surface × state ×
 * viewport and evaluates a battery of DOM-geometry rules IN-PAGE (over the real
 * rendered box model via getBoundingClientRect / getComputedStyle). Every rule
 * maps to a taxonomy class marked `[G]`:
 *
 *   A1 zero-gap adjacency   A2 overlap          A3 protrusion
 *   A4 uneven sibling gap    A7 off-grid spacing  A8 row vertical-centering
 *   A9 peer metric mismatch  A11 border clipped by ancestor
 *   A12 cramped nested borders (double border ≤8px)
 *   B1 premature wrap/stack  B2 failure-to-wrap  B3 h-overflow
 *   B6 fixed > viewport      B8 mid-word break
 *   C7 indistinguishable roles  C9 icon/label split  C10 icon disproportion
 *   C12 placeholder element
 *   D1 truncated-with-room
 *   G5 tap-target < 44 (mobile)  G7 clipped focus-ring / elevation shadow
 *   H7 empty picker dropdown (0 options, no empty-hint)
 *   I1 z-collision (hit-test)    I4 modal-in-viewport   I5 wrong-scroll-axis (strip)
 *   J6 mixed button variants     J7 same-action control side (cross-surface)
 *   K1 persistent context inside scroll
 *   L1 math  L2 mermaid  L3 syntax-highlight  L4 table/footnote  L5 malformed-fallback
 *   H6 broken image
 *
 * Because these are MEASURED off the rendered DOM, a defect can't be "missed" the
 * way an eyeball review misses it: if the geometry is wrong, the number is wrong.
 *
 * Output:
 *   - src/dev/gallery/GEOMETRY_FINDINGS.jsonl  (one finding/line — machine feed)
 *   - src/dev/gallery/GEOMETRY_FINDINGS.md     (grouped human summary)
 *
 * Allow-list: src/dev/gallery/geometry-allowlist.json — array of
 *   { "class":"A1", "surface":"…"|"*", "selector":"<substr>"|"", "viewport":"…"?, "reason":"…" }.
 *   A HIGH finding matching an entry is excused (still reported, marked `allowed`).
 *
 * Gate: `--gate` exits non-zero iff any HIGH finding is NOT allow-listed. Only a
 * small, near-zero-false-positive set is HIGH (A1 real zero-gap, B3 page overflow,
 * B1 genuine premature WRAP). Everything else is MEDIUM/LOW review signal.
 *
 * Usage:
 *   node scripts/gallery-geometry-audit.mjs [--url=BASE] [--gate]
 *        [--viewports=mobile,tablet,desktop] [--states=loaded,empty,error]
 *        [--classes=A1,B1,…] [--concurrency=6] [--out=DIR] [--preview]
 */
import { chromium } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const UI_DIR = path.resolve(__dirname, '..')
const GALLERY_DIR = path.resolve(UI_DIR, 'src/dev/gallery')

const arg = (n, d) =>
  (process.argv.find(a => a.startsWith(`--${n}=`)) || `--${n}=${d}`)
    .split('=')
    .slice(1)
    .join('=')
const flag = n => process.argv.includes(`--${n}`)

const PORT = process.env.GALLERY_PORT || '1420'
const BASE = arg('url', `http://localhost:${PORT}/dev-gallery.html`)
const OUT = arg('out', GALLERY_DIR)
const GATE = flag('gate')
const PREVIEW = flag('preview') // tag the run mode (L3 shiki dev-vs-preview)
const CONCURRENCY = Number(arg('concurrency', '6'))
const CLASS_FILTER = arg('classes', '')
  .split(',')
  .map(s => s.trim())
  .filter(Boolean)
const ALLOWLIST_PATH = path.join(GALLERY_DIR, 'geometry-allowlist.json')

const ALL_VIEWPORTS = [
  { name: 'mobile', width: 390, height: 844 },
  { name: 'tablet', width: 768, height: 1024 },
  { name: 'desktop', width: 1280, height: 900 },
]
const VIEWPORTS = ALL_VIEWPORTS.filter(v =>
  arg('viewports', 'mobile,tablet,desktop').split(',').includes(v.name),
)
const PAGE_STATES = arg('states', 'loaded,empty,error').split(',').filter(Boolean)

import { pathToFileURL } from 'node:url'

const SEVERITY_RANK = { HIGH: 3, MEDIUM: 2, LOW: 1 }
/**
 * Per-class default severity. HIGH is reserved for near-zero-false-positive
 * classes so the `--gate` is a meaningful cannot-miss tripwire; the rich review
 * signal lives in MEDIUM/LOW (still fully reported = still "flagged").
 */
export const CLASS_SEVERITY = {
  A1: 'HIGH', // real zero-gap between content pills (segmented controls excluded)
  A2: 'MEDIUM',
  A3: 'MEDIUM',
  A4: 'LOW',
  A5: 'MEDIUM', // asymmetric vertical padding around input content (off-center)
  A7: 'LOW',
  A8: 'MEDIUM',
  A9: 'MEDIUM',
  A10: 'MEDIUM', // form control at zero/near-zero size (the "input disappears" class)
  A11: 'MEDIUM', // bordered element whose border is clipped by an overflow ancestor
  A12: 'LOW', // cramped double-border (edge-adjacent outline control)
  A13: 'MEDIUM', // child block breaks the parent's alignment axis (left block in a right-aligned message)
  A14: 'LOW', // dead space from an over-tall min/fixed height (content fills far less than the box)
  C1: 'MEDIUM', // status badge ordered before its label
  G9: 'MEDIUM', // hover-only controls reserve no space → persistent sibling shifts
  H7: 'MEDIUM', // empty select/combobox trigger renders nothing
  B1: 'MEDIUM', // per-finding upgraded to HIGH for genuine flex-WRAP-with-room
  B2: 'MEDIUM',
  B3: 'HIGH',
  B6: 'MEDIUM',
  B8: 'LOW',
  C7: 'MEDIUM',
  C9: 'MEDIUM',
  C10: 'MEDIUM',
  C12: 'MEDIUM',
  D1: 'MEDIUM',
  G5: 'LOW',
  G7: 'MEDIUM',
  I1: 'MEDIUM',
  I4: 'HIGH',
  I5: 'MEDIUM',
  J6: 'MEDIUM',
  J7: 'MEDIUM',
  K1: 'MEDIUM',
  L1: 'MEDIUM',
  L2: 'MEDIUM',
  L3: 'MEDIUM',
  L4: 'MEDIUM',
  L5: 'HIGH',
  H6: 'MEDIUM',
  H7: 'MEDIUM',
}

// Context-level testids that must NOT live inside a message/content scroll
// container (K1). Kept in sync with the app's conversation chrome.
export const CONTEXT_TESTIDS = [
  'project-header-chip-tag', // the "In project: …" conversation chip
  'conversation-project-chip',
  'conversation-title',
  'conversation-status',
  'chat-mode-indicator',
  'conversation-model-indicator',
]
// Action-control testids whose SIDE (left/right) must be consistent across
// containers (J7). Matched as substrings.
export const ACTION_SIDE_TOKENS = [
  'expand', 'collapse', 'fullscreen', 'close', 'copy', 'download',
]

// ───────────────────────────────────────────────────────────────────────────
// The in-page geometry audit. Self-contained (serialized into the page).
// Returns { findings:[{cls,severity?,selector,detail,nums}], actionSides:[{action,side,selector}] }.
// ───────────────────────────────────────────────────────────────────────────
export function inPageGeometry({ classesArg, contextTestids, actionTokens, preview }) {
  const run = c => !classesArg.length || classesArg.includes(c)
  const findings = []
  const push = (cls, selector, detail, nums, severity) =>
    findings.push({ cls, selector, detail, nums: nums || {}, severity })
  const actionSides = []

  const VW = window.innerWidth
  const VH = window.innerHeight

  const isChrome = el =>
    !el ||
    !!el.closest?.('[data-gallery-chrome]') ||
    !!el.closest?.('[data-testid="gallery-controls"]')
  const inSvg = el => !!el.closest?.('svg')
  const cs = el => getComputedStyle(el)
  const rectOf = el => el.getBoundingClientRect()
  const visible = el => {
    if (!el || el.nodeType !== 1) return false
    const s = cs(el)
    if (s.visibility === 'hidden' || s.display === 'none' || s.opacity === '0')
      return false
    const r = rectOf(el)
    return r.width >= 1 && r.height >= 1
  }
  const hasTextNode = el =>
    Array.from(el.childNodes).some(n => n.nodeType === 3 && n.textContent.trim())
  const textOf = el => (el.textContent || '').trim()
  const clsOf = el => (typeof el.className === 'string' ? el.className : '')

  const selectorFor = el => {
    if (!el || el.nodeType !== 1) return '?'
    const tid = el.getAttribute?.('data-testid')
    if (tid) return `[data-testid="${tid}"]`
    const slot = el.getAttribute?.('data-slot')
    const id = el.id ? `#${el.id}` : ''
    const c = clsOf(el)
      ? '.' + clsOf(el).trim().split(/\s+/).slice(0, 2).join('.')
      : ''
    return `${el.tagName.toLowerCase()}${id}${slot ? `[data-slot=${slot}]` : ''}${c}`.slice(0, 90)
  }

  // General element pool: visible, non-chrome, NOT inside an <svg> (svg internals
  // are icon geometry, not layout — they were the dominant false-positive source).
  const pool = Array.from(document.querySelectorAll('body *')).filter(
    el => !isChrome(el) && !inSvg(el) && visible(el),
  )
  const parentsOf = () => {
    const set = new Set()
    for (const el of pool) if (el.parentElement && !inSvg(el.parentElement)) set.add(el.parentElement)
    return set
  }

  const clipAxes = el => {
    const s = cs(el)
    const clip = v => v === 'hidden' || v === 'clip' || v === 'auto' || v === 'scroll'
    return { x: clip(s.overflowX), y: clip(s.overflowY) }
  }
  const hasClipAncestorX = el => {
    let n = el.parentElement
    while (n && n !== document.body) {
      if (clipAxes(n).x) return true
      n = n.parentElement
    }
    return false
  }
  // True iff some ancestor is an ACTUALLY horizontally-scrolling container: its
  // overflow-x is auto/scroll AND its content genuinely overflows it. Used to
  // treat scroll-afforded spill (code blocks, wide tables) as non-defects while
  // NOT suppressing genuine protrusions clipped only by a page-level hidden root.
  const hasScrollingAncestorX = el => {
    let n = el.parentElement
    while (n && n !== document.body) {
      const ox = cs(n).overflowX
      if ((ox === 'auto' || ox === 'scroll') && n.scrollWidth - n.clientWidth > 2) return true
      n = n.parentElement
    }
    return false
  }
  const clippingAncestor = (el, axis) => {
    let n = el.parentElement
    while (n && n !== document.body && n !== document.documentElement) {
      if (clipAxes(n)[axis]) return n
      n = n.parentElement
    }
    return null
  }
  // Visually-hidden / decorative-clipped elements (sr-only headings, a11y text
  // mirrors, the Base-UI progress `role=presentation` value span with a
  // clip-path inset) are INTENTIONALLY clipped — they are never a real
  // "truncated-with-room" (D1) defect. Detect the standard hiding signatures.
  const isVisuallyHidden = el => {
    if (/\bsr-only\b/.test(clsOf(el))) return true
    const s = cs(el)
    if (s.clip === 'rect(0px, 0px, 0px, 0px)') return true
    if (s.clipPath && s.clipPath !== 'none' && /inset\(/.test(s.clipPath) &&
      (el.getAttribute('role') === 'presentation' || el.getAttribute('aria-hidden') === 'true')) return true
    const r = rectOf(el)
    if (s.position === 'absolute' && (r.width <= 1 || r.height <= 1)) return true
    return false
  }

  // A "content pill" for A1 = badge/tag/chip/button/link leaf carrying content.
  const PILL_SEL =
    'button,a[href],[role="button"],[role="link"],[data-slot="badge"],[data-slot="tag"],[data-slot="button"]'
  const looksLikePill = el =>
    el.matches?.(PILL_SEL) || /\b(badge|tag|chip|pill)\b/.test(clsOf(el).toLowerCase())
  // Segmented / joined / tablist containers where zero-gap is BY DESIGN.
  const isSegmentedContainer = el => {
    const role = el.getAttribute?.('role')
    if (role === 'tablist' || role === 'radiogroup' || role === 'group') return true
    const slot = el.getAttribute?.('data-slot') || ''
    if (/segment|tabs-list|toggle-group|join/.test(slot)) return true
    const c = clsOf(el).toLowerCase()
    if (/segment|\bjoin\b|toggle-group|inline-flex/.test(c) && /rounded/.test(c)) return true
    const tid = (el.getAttribute?.('data-testid') || '').toLowerCase()
    if (/segment|-opt-|toggle/.test(tid)) return true
    return false
  }

  const INTERACTIVE =
    'button,a[href],input:not([type="hidden"]),select,textarea,[role="button"],[role="link"],[role="checkbox"],[role="switch"],[role="tab"],[role="menuitem"],[role="radio"]'
  const FOCUSABLE =
    'button,a[href],input:not([type="hidden"]),select,textarea,[tabindex]:not([tabindex="-1"]),[role="button"],[role="link"],[role="tab"],[role="checkbox"],[role="switch"],[role="menuitem"]'

  // ── color helpers (for C7 effective-background comparison) ────────────────
  const canvas = document.createElement('canvas')
  canvas.width = canvas.height = 1
  const cctx = canvas.getContext('2d', { willReadFrequently: true })
  const parseColor = c => {
    if (!c || c === 'transparent') return { r: 0, g: 0, b: 0, a: 0 }
    try {
      cctx.clearRect(0, 0, 1, 1)
      cctx.fillStyle = '#000'
      cctx.fillStyle = c
      cctx.fillRect(0, 0, 1, 1)
      const [r, g, b, a] = cctx.getImageData(0, 0, 1, 1).data
      return { r, g, b, a: a / 255 }
    } catch {
      return null
    }
  }
  const over = (fg, bg) => ({
    r: fg.r * fg.a + bg.r * (1 - fg.a),
    g: fg.g * fg.a + bg.g * (1 - fg.a),
    b: fg.b * fg.a + bg.b * (1 - fg.a),
    a: 1,
  })
  const pageBase = () => {
    for (const el of [document.documentElement, document.body]) {
      const c = parseColor(cs(el).backgroundColor)
      if (c && c.a > 0) return { r: c.r, g: c.g, b: c.b, a: 1 }
    }
    return document.documentElement.classList.contains('dark')
      ? { r: 10, g: 10, b: 10, a: 1 }
      : { r: 255, g: 255, b: 255, a: 1 }
  }
  const PAGE = pageBase()
  const effBg = el => {
    let base = { ...PAGE }
    const stack = []
    let n = el
    while (n && n.nodeType === 1) {
      const c = parseColor(cs(n).backgroundColor)
      if (c && c.a > 0) stack.push(c)
      n = n.parentElement
    }
    for (let i = stack.length - 1; i >= 0; i--) base = over(stack[i], base)
    return base
  }
  const bgKey = c => `${Math.round(c.r / 8)},${Math.round(c.g / 8)},${Math.round(c.b / 8)}`

  // ── A1 zero-gap adjacency ────────────────────────────────────────────────
  if (run('A1')) {
    for (const parent of parentsOf()) {
      if (isChrome(parent) || isSegmentedContainer(parent)) continue
      const ps = cs(parent)
      if (parseFloat(ps.columnGap || '0') >= 2) continue
      if (/space-x-|divide-x/.test(clsOf(parent))) continue
      const kids = Array.from(parent.children).filter(
        c => visible(c) && !inSvg(c) && looksLikePill(c) && (hasTextNode(c) || c.querySelector('svg,img')),
      )
      if (kids.length < 2) continue
      kids.sort((a, b) => rectOf(a).left - rectOf(b).left)
      for (let i = 0; i < kids.length - 1; i++) {
        const a = rectOf(kids[i])
        const b = rectOf(kids[i + 1])
        const sameRow = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top) > 4
        const gap = b.left - a.right
        if (sameRow && gap >= -1 && gap < 2) {
          push(
            'A1',
            `${selectorFor(kids[i])} + ${selectorFor(kids[i + 1])}`,
            `zero-gap adjacency: "${textOf(kids[i]).slice(0, 16)}" and "${textOf(kids[i + 1]).slice(0, 16)}" touch (gap ${gap.toFixed(1)}px), parent has no gap/space-x`,
            { gap: +gap.toFixed(1) },
          )
        }
      }
    }
  }

  // ── A2 sibling overlap (non-svg, non-absolute) ───────────────────────────
  if (run('A2')) {
    for (const parent of parentsOf()) {
      const kids = Array.from(parent.children).filter(
        c => visible(c) && !inSvg(c) && !['absolute', 'fixed', 'sticky'].includes(cs(c).position),
      )
      for (let i = 0; i < kids.length; i++)
        for (let j = i + 1; j < kids.length; j++) {
          const a = rectOf(kids[i]), b = rectOf(kids[j])
          const ix = Math.max(0, Math.min(a.right, b.right) - Math.max(a.left, b.left))
          const iy = Math.max(0, Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top))
          const inter = ix * iy
          const minA = Math.min(a.width * a.height, b.width * b.height)
          if (inter > 40 && minA > 0 && inter / minA > 0.25) {
            push(
              'A2',
              `${selectorFor(kids[i])} ∩ ${selectorFor(kids[j])}`,
              `sibling boxes overlap by ${Math.round(inter)}px² (${Math.round((100 * inter) / minA)}% of smaller)`,
              { overlapPx2: Math.round(inter) },
            )
          }
        }
    }
  }

  // ── A3 protrusion ─────────────────────────────────────────────────────────
  if (run('A3')) {
    for (const el of pool) {
      const parent = el.parentElement
      if (!parent || isChrome(parent) || inSvg(parent)) continue
      if (['absolute', 'fixed'].includes(cs(el).position)) continue
      if (clipAxes(parent).x) continue
      // A3 = protrusion WITHOUT an overflow affordance. If an ancestor is an
      // ACTUALLY horizontally-scrolling container (overflow-x auto/scroll AND its
      // content genuinely overflows — e.g. a code block's `overflow-x-auto`
      // wrapping long Shiki lines), the spill is scrollable, not a visual break.
      // Guarded on scrollWidth>clientWidth so a page-level `overflow-x:hidden`
      // root never suppresses genuine button/row protrusions.
      if (hasScrollingAncestorX(el)) continue
      // display:contents / layout roots have no real box → child "protrudes"
      // spuriously. Skip contents parents and page-layout containers.
      const pd = cs(parent).display
      if (pd === 'contents' || pd === 'inline') continue
      if (['MAIN', 'FORM', 'FIELDSET', 'BODY', 'HTML'].includes(parent.tagName)) continue
      if (parent.clientWidth < 8) continue
      const m = cs(el)
      if (parseFloat(m.marginLeft) < -1 || parseFloat(m.marginRight) < -1) continue
      const a = rectOf(el), p = rectOf(parent)
      const out = Math.max(a.right - p.right, p.left - a.left)
      if (out > 3 && a.width < VW) {
        push(
          'A3',
          selectorFor(el),
          `protrudes ${Math.round(out)}px past parent ${selectorFor(parent)} (no overflow clip)`,
          { protrudePx: Math.round(out) },
        )
      }
    }
  }

  // ── A4 uneven like-sibling gaps (LOW) ────────────────────────────────────
  if (run('A4')) {
    for (const parent of parentsOf()) {
      const ps = cs(parent)
      if (parseFloat(ps.rowGap || '0') >= 2 && ps.display.includes('flex')) continue
      const kids = Array.from(parent.children).filter(c => visible(c) && !inSvg(c))
      if (kids.length < 3) continue
      const tag = kids[0].tagName
      if (!kids.every(k => k.tagName === tag)) continue
      const rects = kids.map(rectOf).sort((a, b) => a.top - b.top)
      if (!rects.every((r, i) => i === 0 || r.top >= rects[i - 1].bottom - 2)) continue
      const gaps = []
      for (let i = 1; i < rects.length; i++) gaps.push(rects[i].top - rects[i - 1].bottom)
      const min = Math.min(...gaps), max = Math.max(...gaps)
      if (max - min > 8 && max < 200)
        push('A4', selectorFor(parent),
          `uneven gaps between ${kids.length} <${tag.toLowerCase()}> siblings: ${gaps.map(g => Math.round(g)).join(',')}px`,
          { range: Math.round(max - min) }, 'LOW')
    }
  }

  // ── A7 off-grid spacing (LOW, aggregate) ─────────────────────────────────
  if (run('A7')) {
    const off = new Set()
    for (const el of pool) {
      const s = cs(el)
      for (const v of [s.paddingTop, s.paddingRight, s.paddingBottom, s.paddingLeft, s.marginTop, s.marginRight, s.marginBottom, s.marginLeft, s.rowGap, s.columnGap]) {
        const px = Math.abs(parseFloat(v))
        if (!px || Number.isNaN(px)) continue
        if (px % 2 > 0.5 && 2 - (px % 2) > 0.5) off.add(+px.toFixed(1))
      }
    }
    if (off.size)
      push('A7', 'body', `${off.size} off-grid (non-2px-step) spacing value(s): ${[...off].sort((a, b) => a - b).slice(0, 12).join(',')}px`, { count: off.size }, 'LOW')
  }

  // ── A8 row-children vertical centering in strips ─────────────────────────
  if (run('A8')) {
    const strips = pool.filter(el => {
      const role = el.getAttribute('role')
      const slot = el.getAttribute('data-slot') || ''
      return role === 'tablist' || role === 'toolbar' || /tabs-list|segment|toolbar/.test(slot)
    })
    for (const strip of strips) {
      const kids = Array.from(strip.children).filter(c => visible(c) && !inSvg(c))
      if (kids.length < 2) continue
      const sr = rectOf(strip)
      const scy = sr.top + sr.height / 2
      for (const k of kids) {
        const kr = rectOf(k)
        const kcy = kr.top + kr.height / 2
        if (Math.abs(kcy - scy) > 2) {
          push('A8', selectorFor(strip),
            `strip child ${selectorFor(k)} center-y off container center by ${Math.round(Math.abs(kcy - scy))}px (vertical mis-centering)`,
            { off: Math.round(Math.abs(kcy - scy)) })
          break
        }
      }
    }
  }

  // ── A9 peer internal-metric inconsistency ────────────────────────────────
  // Same-kind icon-bearing peers in one group (footer chips, toolbar buttons,
  // stat pills) must share internal metrics (element height, icon box size). A
  // peer deviating from the group mode is flagged — the chat composer footer
  // chips ("Memory: auto" / "Summary: auto") render at different apparent sizes
  // because their icons differ (taxonomy A9, user miss #15).
  if (run('A9')) {
    const keyOf = el => {
      const tid = (el.getAttribute('data-testid') || '').replace(/[-_][^-_]*\d[^-_]*$/, '').replace(/\d+/g, '#')
      const cls0 = clsOf(el).trim().split(/\s+/)[0] || ''
      return `${el.tagName}|${cls0}|${tid}`
    }
    for (const parent of parentsOf()) {
      const kids = Array.from(parent.children).filter(c => visible(c) && !inSvg(c) && c.querySelector('svg,img'))
      if (kids.length < 3) continue
      const groups = {}
      for (const k of kids) (groups[keyOf(k)] ??= []).push(k)
      for (const peers of Object.values(groups)) {
        if (peers.length < 3) continue
        const metric = el => {
          const r = rectOf(el)
          const icon = el.querySelector('svg,img')
          const ir = icon ? icon.getBoundingClientRect() : null
          return { h: Math.round(r.height), w: Math.round(r.width), ih: ir ? Math.round(ir.height) : 0, iw: ir ? Math.round(ir.width) : 0 }
        }
        const ms = peers.map(metric)
        // The A9 target is uniform CHIP/toolbar-button/stat-pill peers (user miss
        // #15). Comparing the "first icon" across large heterogeneous containers
        // (e.g. `chat-message` rows, where a text message's inline 16px icon is
        // measured against a tool-call row's 24px status icon) is a false
        // positive — different roles legitimately carry different icons. Only
        // judge icon-box metrics when the peers are themselves chip/control-sized.
        const chipLike = ms.every(m => m.h > 0 && m.h <= 48) && ms.every(m => m.w <= 320)
        for (const field of ['h', 'ih', 'iw']) {
          if ((field === 'ih' || field === 'iw') && !chipLike) continue
          const vals = ms.map(m => m[field]).filter(v => v > 0)
          if (vals.length < 3) continue
          const freq = {}
          for (const v of vals) freq[v] = (freq[v] || 0) + 1
          const mode = +Object.entries(freq).sort((a, b) => b[1] - a[1])[0][0]
          if (freq[mode] < peers.length / 2) continue // no clear mode → skip
          // element-height legitimately varies by content for CARDS/rows; only
          // judge it for chip/button-sized peers (mode < 48px).
          if (field === 'h' && mode >= 48) continue
          peers.forEach((p, i) => {
            const v = ms[i][field]
            if (v > 0 && Math.abs(v - mode) > 2) {
              push('A9', selectorFor(p),
                `peer metric mismatch (${field === 'h' ? 'element-height' : field === 'ih' ? 'icon-height' : 'icon-width'}): ${v}px vs group mode ${mode}px among ${peers.length} same-kind siblings`,
                { field, v, mode })
            }
          })
        }
      }

      // Menu / dropdown / popover ITEMS: leading-icon + text-label rows that are
      // actionable (role menuitem/button/link/option, a menu-ish testid/slot, a
      // tabindex, or cursor:pointer). They carry PER-ITEM testids, so the keyOf
      // grouping above never groups them — group ALL such rows in ONE parent and
      // require their LEADING icon boxes to match. This closes the coverage gap
      // where the composer "+" menu "Skills in this chat" item renders its icon at
      // lucide's 24px default among 16px (size-4) peers, unscanned because the
      // menu is interaction-gated + the rows have distinct testids.
      const isMenuItemRow = el => {
        if (!el.querySelector('svg,img') || !textOf(el)) return false
        const role = el.getAttribute('role')
        const tid = el.getAttribute('data-testid') || ''
        const slot = el.getAttribute('data-slot') || ''
        // A STRONG menu signal only — not a bare role=button / cursor:pointer (those
        // match ordinary message rows / clickable cards and produced a flood).
        const strong =
          role === 'menuitem' || role === 'option' ||
          /menu-?item|menu-trigger|menu-option|dropdown-item|cmdk-item/i.test(tid) ||
          /menu-?item|dropdown-menu-item|command-item/i.test(slot)
        if (strong) return true
        // …or a plain icon+label row that lives INSIDE a menu/dropdown/popover/command
        // surface (the composer "+" items are custom rows inside a Popover).
        return !!el.closest(
          '[role="menu"],[role="listbox"],[data-slot*="menu"],[data-slot*="popover"],[data-slot*="dropdown"],[data-radix-popper-content-wrapper],[cmdk-list]',
        )
      }
      const menuItems = Array.from(parent.children).filter(
        c => visible(c) && !inSvg(c) && isMenuItemRow(c),
      )
      if (menuItems.length >= 3) {
        const boxes = menuItems.map(el => {
          const ic = el.querySelector('svg,img')
          return ic ? ic.getBoundingClientRect() : null
        })
        for (const [field, prop] of [['ih', 'height'], ['iw', 'width']]) {
          const vals = boxes.map(bx => (bx ? Math.round(bx[prop]) : 0)).filter(v => v > 0)
          if (vals.length < 3) continue
          const freq = {}
          for (const v of vals) freq[v] = (freq[v] || 0) + 1
          const mode = +Object.entries(freq).sort((a, b) => b[1] - a[1])[0][0]
          if (freq[mode] < menuItems.length / 2) continue
          menuItems.forEach((mi, i) => {
            const bx = boxes[i]
            if (!bx) return
            const v = Math.round(bx[prop])
            if (v > 0 && Math.abs(v - mode) > 2) {
              push('A9', selectorFor(mi),
                `menu-item leading-icon ${field === 'ih' ? 'height' : 'width'} ${v}px vs group mode ${mode}px among ${menuItems.length} menu items`,
                { field, v, mode, menu: true })
            }
          })
        }
      }
    }
  }

  // ── B1 premature wrap / premature stack ──────────────────────────────────
  if (run('B1')) {
    for (const el of pool) {
      const s = cs(el)
      if (!s.display.includes('flex')) continue
      const className = clsOf(el)
      const isWrap = s.flexWrap === 'wrap' || s.flexWrap === 'wrap-reverse'
      const respRow = /\b(sm|md|lg):flex-row\b/.test(className)
      const caseWrap = isWrap && s.flexDirection.startsWith('row')
      const caseStack = respRow && s.flexDirection === 'column'
      if (!caseWrap && !caseStack) continue
      const kids = Array.from(el.children).filter(c => visible(c) && !inSvg(c))
      if (kids.length < 2 || kids.length > 6) continue
      const rects = kids.map(rectOf)
      const mids = rects.map(r => Math.round((r.top + r.bottom) / 2))
      const bands = []
      for (const m of mids) if (!bands.some(bb => Math.abs(bb - m) < 8)) bands.push(m)
      if (bands.length < 2) continue
      const colGap = parseFloat(s.columnGap || '0') || parseFloat(s.gap || '0') || 8
      const sumW = rects.reduce((a, r) => a + r.width, 0) + colGap * (kids.length - 1)
      const inner = el.clientWidth
      const slack = inner - sumW
      if (slack >= 16) {
        // Genuine premature WRAP (flex-wrap) is HIGH — content wrapped with room,
        // a hard layout bug. The responsive-STACK case is a documented mobile
        // tradeoff (Card header) → MEDIUM review signal.
        push('B1', selectorFor(el),
          `premature ${caseStack ? 'stack' : 'wrap'}: ${kids.length} children on ${bands.length} rows but Σwidths+gaps ≈ ${Math.round(sumW)}px ≤ container ${inner}px (${Math.round(slack)}px slack — fits on one row)`,
          { needW: Math.round(sumW), innerW: inner, slack: Math.round(slack) },
          caseWrap ? 'HIGH' : 'MEDIUM')
      }
    }
  }

  // ── B2 failure-to-wrap ────────────────────────────────────────────────────
  if (run('B2')) {
    for (const el of pool) {
      if (!hasTextNode(el)) continue
      const s = cs(el)
      if (s.whiteSpace !== 'nowrap' && s.whiteSpace !== 'pre') continue
      if (el.scrollWidth <= el.clientWidth + 2) continue
      if (s.textOverflow === 'ellipsis' || clipAxes(el).x) continue
      push('B2', selectorFor(el), `nowrap text overflows by ${el.scrollWidth - el.clientWidth}px without wrap or ellipsis`, { overflowPx: el.scrollWidth - el.clientWidth })
    }
  }

  // ── B3 horizontal overflow at the viewport (HIGH) ────────────────────────
  if (run('B3')) {
    const docW = document.documentElement.scrollWidth
    if (docW > VW + 2) {
      let worst = null
      for (const el of pool) {
        const r = rectOf(el)
        if (r.right > VW + 2 && r.width < VW * 1.5 && !hasClipAncestorX(el)) {
          const over = r.right - VW
          if (!worst || over > worst.over) worst = { el, over }
        }
      }
      push('B3', worst ? selectorFor(worst.el) : 'html',
        `document scrollWidth ${docW}px > viewport ${VW}px → horizontal scrollbar${worst ? ` (worst crosses right edge by ${Math.round(worst.over)}px)` : ''}`,
        { docW, vw: VW })
    }
  }

  // ── B6 element wider than viewport (skip scroll-container children) ───────
  if (run('B6')) {
    for (const el of pool) {
      if (el === document.body || el === document.documentElement) continue
      const r = rectOf(el)
      if (r.width > VW + 2 && r.width < VW * 3 && !clipAxes(el).x && !hasClipAncestorX(el)) {
        push('B6', selectorFor(el), `element width ${Math.round(r.width)}px exceeds viewport ${VW}px`, { w: Math.round(r.width), vw: VW })
      }
    }
  }

  // ── B8 mid-word break (LOW) ──────────────────────────────────────────────
  if (run('B8')) {
    for (const el of pool) {
      const t = textOf(el)
      if (!t || /\s/.test(t) || t.length < 16) continue
      if (Array.from(el.childNodes).some(n => n.nodeType === 1)) continue
      const s = cs(el)
      const overflowing = el.scrollWidth > el.clientWidth + 2
      const wrapping = rectOf(el).height > parseFloat(s.lineHeight || '0') * 1.6
      if ((overflowing || wrapping) && !/break-word|anywhere|break-all/.test(s.overflowWrap + s.wordBreak))
        push('B8', selectorFor(el), `long token "${t.slice(0, 20)}…" (${t.length} chars) ${wrapping ? 'wraps' : 'overflows'} without break-word intent`, { len: t.length }, 'LOW')
    }
  }

  // ── C7 indistinguishable roles ───────────────────────────────────────────
  if (run('C7')) {
    // group elements that carry an explicit role variant
    const roleEls = pool.filter(el => el.hasAttribute('data-role'))
    const byRole = {}
    for (const el of roleEls) {
      const role = el.getAttribute('data-role')
      ;(byRole[role] ??= []).push(el)
    }
    const roles = Object.keys(byRole)
    if (roles.length >= 2) {
      const sig = el => {
        const parent = el.parentElement
        const pr = parent ? rectOf(parent) : { left: 0, width: VW }
        const r = rectOf(el)
        // alignment bucket within parent
        const leftPad = r.left - pr.left
        const rightPad = pr.left + pr.width - r.right
        const align = Math.abs(leftPad - rightPad) < 12 ? 'center' : leftPad < rightPad ? 'left' : 'right'
        // an avatar/decoration counts only if it actually paints content
        const av = el.querySelector('[class*="rounded-full"]')
        const avVisible = !!(av && visible(av) && (av.querySelector('img,svg') || textOf(av)))
        const bg = bgKey(effBg(el))
        const s = cs(el)
        const borderVisible = parseFloat(s.borderTopWidth) > 0 || parseFloat(s.borderLeftWidth) > 0
        return `bg=${bg}|align=${align}|border=${borderVisible ? 1 : 0}|avatar=${avVisible ? 1 : 0}`
      }
      for (let i = 0; i < roles.length; i++)
        for (let j = i + 1; j < roles.length; j++) {
          const a = byRole[roles[i]][0], b = byRole[roles[j]][0]
          if (!a || !b) continue
          const sa = sig(a), sb = sig(b)
          if (sa === sb) {
            push('C7', `[data-role="${roles[i]}"] vs [data-role="${roles[j]}"]`,
              `two DIFFERENT roles ("${roles[i]}" vs "${roles[j]}") render with an IDENTICAL visual signature (${sa}) — reader can't tell them apart`,
              {})
          }
        }
    }
  }

  // ── C9 icon/label pair split across lines ────────────────────────────────
  if (run('C9')) {
    const srOnly = el => {
      if (/\bsr-only\b/.test(clsOf(el))) return true
      const s = cs(el)
      const r = rectOf(el)
      return (s.position === 'absolute' && (r.width <= 1 || r.height <= 1)) || s.clip === 'rect(0px, 0px, 0px, 0px)'
    }
    const leafIcons = Array.from(document.querySelectorAll('svg,img')).filter(el => visible(el) && !isChrome(el))
    for (const icon of leafIcons) {
      const parent = icon.parentElement
      if (!parent) continue
      // Only judge contexts INTENDED to be a horizontal icon+label row: parent is
      // a flex row (not flex-col — vertical empty-state stacks are legitimate) or
      // an inline label/button/alert-title context.
      const ps = cs(parent)
      const horizontalIntent = ps.display.includes('flex') && !ps.flexDirection.startsWith('column')
      if (!horizontalIntent) continue
      const sibs = Array.from(parent.children).filter(c => c !== icon && visible(c) && textOf(c) && !srOnly(c))
      const label = sibs[0]
      if (!label) continue
      const ir = rectOf(icon), lr = rectOf(label)
      const yOverlap = Math.min(ir.bottom, lr.bottom) - Math.max(ir.top, lr.top)
      if (yOverlap > 2) continue // on the same line — fine
      const gap = 6
      if (ir.width + lr.width + gap <= parent.clientWidth) {
        push('C9', selectorFor(parent),
          `icon and its label "${textOf(label).slice(0, 20)}" render on different lines (disjoint y) though ${Math.round(ir.width + lr.width + gap)}px fits container ${parent.clientWidth}px`,
          { need: Math.round(ir.width + lr.width + gap), inner: parent.clientWidth })
      }
    }
  }

  // ── C10 icon disproportionate to adjacent text ───────────────────────────
  if (run('C10')) {
    const leafIcons = Array.from(document.querySelectorAll('svg,img')).filter(el => visible(el) && !isChrome(el))
    for (const icon of leafIcons) {
      const parent = icon.parentElement
      if (!parent) continue
      const textSib = Array.from(parent.children).find(c => c !== icon && visible(c) && textOf(c)) ||
        (hasTextNode(parent) ? parent : null)
      if (!textSib) continue
      const lh = parseFloat(cs(textSib).lineHeight) || parseFloat(cs(textSib).fontSize) * 1.4
      if (!lh) continue
      const ih = rectOf(icon).height
      const ratio = ih / lh
      if (ratio > 1.6 || ratio < 0.6) {
        push('C10', selectorFor(icon),
          `icon height ${Math.round(ih)}px is ${ratio.toFixed(2)}× the adjacent text line-height ${Math.round(lh)}px (${ratio > 1.6 ? 'oversized' : 'undersized'})`,
          { ratio: +ratio.toFixed(2) })
      }
    }
  }

  // ── C12 placeholder element (bare avatar circle / placeholder text) ──────
  if (run('C12')) {
    for (const el of pool) {
      const s = cs(el)
      const round = /rounded-full/.test(clsOf(el)) || parseFloat(s.borderRadius) >= Math.min(rectOf(el).width, rectOf(el).height) / 2 - 1
      if (!round) continue
      const r = rectOf(el)
      // Avatar-sized only (≥20px). Below that it's a status dot / indicator, an
      // intentional decoration, not a placeholder for an image/initials.
      if (r.width < 20 || r.width > 96 || Math.abs(r.width - r.height) > 4) continue
      if (el.getAttribute('role') === 'status' || el.getAttribute('aria-label')) continue
      const hasContent = el.querySelector('img,svg') || textOf(el)
      const hasFill = parseFloat(s.borderTopWidth) > 0 || (parseColor(s.backgroundColor)?.a || 0) > 0.05
      if (!hasContent && hasFill) {
        push('C12', selectorFor(el), `bare placeholder circle: rounded-full ${Math.round(r.width)}×${Math.round(r.height)}px with no img/svg/initials content`, {})
      }
    }
    // placeholder tokens in visible text
    const bodyText = document.body.innerText || ''
    const m = bodyText.match(/\b(lorem ipsum|TODO|FIXME|xxxx+)\b|\{\{?\s*[a-zA-Z_]+\s*\}?\}/)
    if (m) push('C12', 'body', `placeholder/unresolved token in rendered text: "${m[0].slice(0, 40)}"`, {}, 'LOW')
  }

  // ── D1 truncated with room ───────────────────────────────────────────────
  if (run('D1')) {
    for (const el of pool) {
      if (isVisuallyHidden(el)) continue
      const s = cs(el)
      const ellipsis = s.textOverflow === 'ellipsis'
      const clipped = (s.overflow === 'hidden' || s.overflowX === 'hidden') && s.whiteSpace === 'nowrap'
      if (!ellipsis && !clipped) continue
      const overflow = el.scrollWidth - el.clientWidth
      if (overflow <= 1) continue
      const parent = el.parentElement
      if (!parent) continue
      const room = parent.clientWidth - el.offsetWidth
      if (room > overflow + 6)
        push('D1', selectorFor(el), `text truncated (hidden ${overflow}px) but parent has ${Math.round(room)}px free — could show "${textOf(el).slice(0, 24)}"`, { hiddenPx: overflow, roomPx: Math.round(room) })
    }
  }

  // ── G5 tap-target < 44px (mobile) ────────────────────────────────────────
  if (run('G5') && VW <= 480) {
    const seen = new Set()
    for (const el of document.querySelectorAll(INTERACTIVE)) {
      if (isChrome(el) || inSvg(el) || !visible(el) || el.getAttribute('aria-hidden') === 'true') continue
      const r = rectOf(el)
      const mn = Math.min(r.width, r.height)
      if (mn < 44) {
        const sel = selectorFor(el)
        if (seen.has(sel)) continue
        seen.add(sel)
        push('G5', sel, `tap target ${Math.round(r.width)}×${Math.round(r.height)}px < 44px (mobile)`, { w: Math.round(r.width), h: Math.round(r.height) }, mn < 24 ? 'MEDIUM' : 'LOW')
      }
    }
  }

  // ── G7 clipped focus-ring / elevation shadow ─────────────────────────────
  if (run('G7')) {
    const measureRing = el => {
      const s = cs(el)
      let ring = 0
      const ow = parseFloat(s.outlineWidth) || 0
      if (ow > 0 && s.outlineStyle !== 'none') ring = ow + Math.max(0, parseFloat(s.outlineOffset) || 0)
      const sh = s.boxShadow && s.boxShadow !== 'none' ? s.boxShadow : ''
      if (sh)
        for (const part of sh.split(/,(?![^(]*\))/)) {
          if (/inset/.test(part)) continue
          const nums = (part.match(/-?\d+(\.\d+)?px/g) || []).map(parseFloat)
          if (nums.length >= 3) {
            const [ox, oy, , spread = 0] = nums
            ring = Math.max(ring, Math.max(Math.abs(ox), Math.abs(oy)) + Math.max(0, spread))
          }
        }
      return ring
    }
    const active = document.activeElement
    for (const el of Array.from(document.querySelectorAll(FOCUSABLE)).slice(0, 140)) {
      if (isChrome(el) || inSvg(el) || !visible(el)) continue
      let ring = 0
      try { el.focus({ preventScroll: true }); ring = measureRing(el) } catch { /* */ }
      // ONLY flag when a real ring is painted (measured). No assumed fallback —
      // that produced thousands of speculative findings.
      if (ring < 1.5) continue
      const r = rectOf(el)
      for (const axis of ['x', 'y']) {
        const anc = clippingAncestor(el, axis)
        if (!anc) continue
        const p = rectOf(anc)
        const cut = axis === 'x'
          ? Math.max(p.left - (r.left - ring), r.right + ring - p.right)
          : Math.max(p.top - (r.top - ring), r.bottom + ring - p.bottom)
        const flush = axis === 'x'
          ? Math.min(r.left - p.left, p.right - r.right) < ring + 1
          : Math.min(r.top - p.top, p.bottom - r.bottom) < ring + 1
        if (cut > 1 && flush) {
          push('G7', selectorFor(el), `focus ring (${ring}px) clipped by overflow-${axis} ancestor ${selectorFor(anc)} — cut ${Math.round(cut)}px`, { ring, cut: Math.round(cut), axis })
          break
        }
      }
    }
    for (const el of document.querySelectorAll('[data-slot="card"]')) {
      if (isChrome(el) || !visible(el)) continue
      const shadow = measureRing(el)
      if (shadow < 3) continue
      for (const axis of ['x', 'y']) {
        const anc = clippingAncestor(el, axis)
        if (!anc) continue
        const p = rectOf(anc), r = rectOf(el)
        const cut = axis === 'x'
          ? Math.max(p.left - (r.left - shadow), r.right + shadow - p.right)
          : Math.max(p.top - (r.top - shadow), r.bottom + shadow - p.bottom)
        if (cut > 2) { push('G7', selectorFor(el), `card elevation shadow (${shadow}px) clipped by overflow-${axis} ancestor — cut ${Math.round(cut)}px`, { shadow, cut: Math.round(cut) }, 'LOW'); break }
      }
    }
    try { if (active?.focus) active.focus({ preventScroll: true }) } catch { /* */ }
  }

  // ── A11 element border clipped by a clipping ancestor ────────────────────
  // Sibling of G7 (reuses the per-axis clipping-ancestor walk): for every element
  // with a real (opaque, ≥1px) border, compare each bordered side's border-box edge
  // to the nearest overflow-clipping ancestor's INNER clip rect (padding box minus
  // scrollbar). If a bordered side sits AT/OUTSIDE that clip edge, the clip cuts the
  // border stroke → the border is being eaten (the classic "card/control border eaten
  // by a negative-margin / overflow-hidden container edge").
  //
  // The discriminator against a genuinely SCROLLED-OUT element (whose edge is also
  // "outside" the clip, but is NOT a defect — it's just off-screen content in a
  // scroll pane) is MAJORITY-VISIBILITY: we only flag an element that is still mostly
  // inside the clip viewport. A row scrolled thousands of px away is ~0% visible and
  // is skipped; an element pinned at the edge with its border poking a few px past is
  // ~full visible and is flagged.
  if (run('A11')) {
    const px = v => parseFloat(v) || 0
    const paintedBorder = (s, name) => {
      const w = px(s[`border${name}Width`])
      if (w < 1) return 0
      const style = s[`border${name}Style`]
      if (style === 'none' || style === 'hidden') return 0
      const col = parseColor(s[`border${name}Color`])
      if (col && col.a === 0) return 0 // transparent border occupies layout but paints nothing
      return w
    }
    // Inner clip rect = the ancestor's padding box minus any scrollbar gutter, i.e.
    // the region overflow actually clips to. Built from computed border widths (sub-
    // pixel) + the client/offset delta (the scrollbar gutter).
    const innerClipRect = anc => {
      const p = rectOf(anc)
      const s = cs(anc)
      const bl = px(s.borderLeftWidth), br = px(s.borderRightWidth)
      const bt = px(s.borderTopWidth), bb = px(s.borderBottomWidth)
      const vGutter = Math.max(0, anc.offsetWidth - anc.clientWidth - bl - br) // vertical scrollbar
      const hGutter = Math.max(0, anc.offsetHeight - anc.clientHeight - bt - bb)
      return { left: p.left + bl, top: p.top + bt, right: p.right - br - vGutter, bottom: p.bottom - bb - hGutter }
    }
    const HUGE = { left: -1e7, top: -1e7, right: 1e7, bottom: 1e7 }
    const LO = 0.75 // ignore sub-pixel rounding of a flush edge
    let scanned = 0
    for (const el of pool) {
      if (scanned > 6000) break
      scanned++
      const s = cs(el)
      const bw = {
        left: paintedBorder(s, 'Left'), right: paintedBorder(s, 'Right'),
        top: paintedBorder(s, 'Top'), bottom: paintedBorder(s, 'Bottom'),
      }
      if (!(bw.left || bw.right || bw.top || bw.bottom)) continue
      const r = rectOf(el)
      if (r.width < 4 || r.height < 4) continue
      const xAnc = clippingAncestor(el, 'x')
      const yAnc = clippingAncestor(el, 'y')
      if ((!xAnc || xAnc === el) && (!yAnc || yAnc === el)) continue
      const cx = xAnc && xAnc !== el ? innerClipRect(xAnc) : HUGE
      const cy = yAnc && yAnc !== el ? innerClipRect(yAnc) : HUGE
      // Majority-visibility: intersection of the element with the (x-clip × y-clip)
      // viewport, as a fraction of the element's own area. Excludes scrolled-out rows.
      const vw = Math.max(0, Math.min(r.right, cx.right) - Math.max(r.left, cx.left))
      const vh = Math.max(0, Math.min(r.bottom, cy.bottom) - Math.max(r.top, cy.top))
      if ((vw * vh) / Math.max(1, r.width * r.height) < 0.5) continue
      const sidesFor = axis => axis === 'x'
        ? [['left', bw.left, cx], ['right', bw.right, cx]]
        : [['top', bw.top, cy], ['bottom', bw.bottom, cy]]
      for (const axis of ['x', 'y']) {
        const anc = axis === 'x' ? xAnc : yAnc
        if (!anc || anc === el) continue
        for (const [side, w, clip] of sidesFor(axis)) {
          if (!w) continue
          const overshoot =
            side === 'left' ? clip.left - r.left
            : side === 'right' ? r.right - clip.right
            : side === 'top' ? clip.top - r.top
            : r.bottom - clip.bottom
          // The clip line is at/inside the border-box edge → the border stroke (and
          // often a sliver of the element) is cut. Report the measured overshoot.
          if (overshoot > LO) {
            push(
              'A11', selectorFor(el),
              `${side} border (${w}px) clipped by overflow-${axis} ancestor ${selectorFor(anc)} — cut ${overshoot.toFixed(1)}px`,
              { side, border: w, cut: +overshoot.toFixed(2), axis },
            )
          }
        }
      }
    }
  }

  // ── A5 asymmetric vertical padding around input content (off-center) ─────
  // [G] form of A5 (was vision-only): a box wrapping an input / editable region
  // whose TOP padding differs from its BOTTOM padding renders the content
  // vertically off-center — it reads uncomfortable/unbalanced. Horizontal padding
  // must be ~symmetric (else it's a deliberate directional layout, not a centering
  // bug). *(the chat composer input area `px-3 pt-2.5 pb-1` = 10px top vs 4px
  // bottom around the text input.)*
  if (run('A5')) {
    const INPUTISH = 'textarea,input:not([type="hidden"]),[contenteditable="true"],[role="textbox"]'
    for (const el of pool) {
      const s = cs(el)
      const pt = parseFloat(s.paddingTop) || 0, pb = parseFloat(s.paddingBottom) || 0
      const pl = parseFloat(s.paddingLeft) || 0, pr = parseFloat(s.paddingRight) || 0
      if (Math.abs(pt - pb) <= 3) continue // symmetric enough
      if (Math.abs(pl - pr) > 3) continue // horizontally asymmetric → directional layout, not centering
      if (pt < 1 && pb < 1) continue
      const input = el.querySelector(INPUTISH)
      if (!input || !visible(input) || inSvg(el)) continue
      const r = rectOf(el)
      if (r.height < 12 || r.height > 260) continue // input row, not a page section
      // The input must DOMINATE the box vertically — else the padding isn't what
      // offsets it (excludes a toolbar/row that merely nests some unrelated input).
      const contentH = r.height - pt - pb
      if (rectOf(input).height < 0.5 * contentH) continue
      push('A5', selectorFor(el),
        `asymmetric vertical padding (top ${Math.round(pt)}px vs bottom ${Math.round(pb)}px) around input content — reads vertically off-center`,
        { pt: Math.round(pt), pb: Math.round(pb) })
    }
  }

  // ── A12 cramped nested borders (a double border with a tight gap) ────────
  // A bordered CONTROL (button / input / select) whose border-box edge sits within
  // ~8px of its nearest bordered ANCESTOR's inner (padding-box) edge reads as a
  // crowded double border — two stroke lines almost touching. The usual fix is a
  // quiet/ghost variant instead of an outline inside an already-bordered container.
  // Scoped to controls (that's where the outline-in-a-box anti-pattern lives) to
  // avoid flagging every full-bleed child of a card.
  if (run('A12')) {
    const px = v => parseFloat(v) || 0
    const paintedBorder = (s, name) => {
      const w = px(s[`border${name}Width`])
      if (w < 1) return 0
      const st = s[`border${name}Style`]
      if (st === 'none' || st === 'hidden') return 0
      const c = parseColor(s[`border${name}Color`])
      if (c && c.a === 0) return 0
      return w
    }
    const anyBorder = el => {
      const s = cs(el)
      return paintedBorder(s, 'Left') || paintedBorder(s, 'Right') || paintedBorder(s, 'Top') || paintedBorder(s, 'Bottom')
    }
    const CONTROL = 'button,a[href],input,select,textarea,[role="button"],[role="link"],[data-slot="button"]'
    // ≤10px: a bordered control this close to its bordered container's edge paints a
    // redundant double stroke (the container border + the control's outline almost
    // parallel). Standard control padding inside a card is ≥12px (px-3), so 10px is
    // the boundary between "comfortably inside" and "crammed double border".
    const GAP = 10
    let scanned = 0
    for (const el of pool) {
      if (scanned > 6000) break
      scanned++
      if (!el.matches?.(CONTROL)) continue
      const es = cs(el)
      const bw = {
        left: paintedBorder(es, 'Left'), right: paintedBorder(es, 'Right'),
        top: paintedBorder(es, 'Top'), bottom: paintedBorder(es, 'Bottom'),
      }
      if (!(bw.left || bw.right || bw.top || bw.bottom)) continue
      let anc = el.parentElement
      while (anc && anc !== document.body && !anyBorder(anc)) anc = anc.parentElement
      if (!anc || anc === document.body || inSvg(anc) || !visible(anc)) continue
      const as = cs(anc)
      const r = rectOf(el), a = rectOf(anc)
      const innerLeft = a.left + px(as.borderLeftWidth), innerRight = a.right - px(as.borderRightWidth)
      const innerTop = a.top + px(as.borderTopWidth), innerBottom = a.bottom - px(as.borderBottomWidth)
      const check = (side, elB, ancB, gap) => {
        if (elB && ancB && gap > -0.5 && gap <= GAP) {
          push('A12', selectorFor(el),
            `${side} border crammed ${gap.toFixed(1)}px from bordered ancestor ${selectorFor(anc)} inner edge — crowded double border (≤${GAP}px); use a quiet/ghost variant, not an outline, inside an already-bordered container`,
            { side, gap: +gap.toFixed(1) })
        }
      }
      check('left', bw.left, paintedBorder(as, 'Left'), r.left - innerLeft)
      check('right', bw.right, paintedBorder(as, 'Right'), innerRight - r.right)
      check('top', bw.top, paintedBorder(as, 'Top'), r.top - innerTop)
      check('bottom', bw.bottom, paintedBorder(as, 'Bottom'), innerBottom - r.bottom)
    }
  }

  // ── A13 child block breaks the parent's alignment axis ───────────────────
  // In a RIGHT-aligned message (the whole bubble pushed right via flex-end /
  // margin-left:auto), a child content block (attachment/file list, action row)
  // that stays LEFT-packed floats at the far left of the message's own width,
  // detached from the bubble it belongs to. Flag a file/attachment/image block
  // whose right edge sits well short of the message's right edge while its left
  // edge hugs the message's left — an alignment mismatch among one message's
  // children. Only right-aligned messages are inspected (left-aligned assistant
  // messages legitimately pack left).
  if (run('A13')) {
    const FILEISH = '[data-testid*="attach"],[data-testid*="file"],[data-testid*="image"],img'
    for (const el of pool) {
      const s = cs(el)
      const parent = el.parentElement
      if (!parent) continue
      const rightAligned = s.alignSelf === 'flex-end' || s.marginLeft === 'auto'
      if (!rightAligned) continue
      const r = rectOf(el)
      if (r.width < 60) continue
      const seen = new Set()
      for (const f of el.querySelectorAll(FILEISH)) {
        if (!visible(f) || inSvg(f)) continue
        const fr = rectOf(f)
        if (fr.width < 8 || fr.width > r.width * 0.9) continue // spans the msg → aligned, skip
        const gapRight = r.right - fr.right
        const leftHug = fr.left - r.left
        if (gapRight > 0.4 * r.width && leftHug < 0.12 * r.width) {
          const key = selectorFor(f)
          if (seen.has(key)) continue
          seen.add(key)
          push('A13', key,
            `left-aligned block inside a right-aligned message ${selectorFor(el)}: its right edge is ${Math.round(gapRight)}px (${Math.round(100 * gapRight / r.width)}% of the message width) from the message's right edge — it detaches from the bubble instead of following its right-alignment`,
            { gapRight: Math.round(gapRight) })
          break // one per message
        }
      }
    }
  }

  // ── A14 dead space from an over-tall min/fixed height ────────────────────
  // A container with an explicit min-height / fixed height whose CONTENT fills
  // far less than the box leaves a large blank band (a small table in a viewer
  // sized for many rows, a short panel pinned to a tall min-height). Flag when
  // (box_height − content_height) exceeds BOTH 35% of the box AND an absolute px
  // floor. Content height = the union extent of the container's visible children.
  if (run('A14')) {
    const FLOOR = 120 // px of empty band before it's worth flagging
    for (const el of pool) {
      const s = cs(el)
      const minH = parseFloat(s.minHeight) || 0
      const hasFixed = s.height !== 'auto' && /px$/.test(s.height)
      if (minH < FLOOR && !hasFixed) continue
      const r = rectOf(el)
      if (r.height < FLOOR) continue
      // Only when the explicit sizing is what makes it tall (content would be shorter).
      const boxH = r.height
      const kids = Array.from(el.children).filter(c => visible(c) && !inSvg(c))
      if (!kids.length) continue
      let top = Infinity, bottom = -Infinity
      for (const c of kids) { const cr = rectOf(c); top = Math.min(top, cr.top); bottom = Math.max(bottom, cr.bottom) }
      const contentH = bottom - top
      if (!(contentH > 0)) continue
      const empty = boxH - contentH
      // scrollable content that's merely clipped isn't dead space
      if (el.scrollHeight > el.clientHeight + 4) continue
      if (empty > 0.35 * boxH && empty > FLOOR) {
        push('A14', selectorFor(el),
          `dead space: content fills ${Math.round(contentH)}px of a ${Math.round(boxH)}px box (${Math.round(empty)}px / ${Math.round(100 * empty / boxH)}% blank) — the ${minH ? `min-height ${Math.round(minH)}px` : 'fixed height'} is too tall for the content`,
          { empty: Math.round(empty), boxH: Math.round(boxH) })
      }
    }
  }

  // ── H7 empty picker — a select/menu dropdown that renders NOTHING ────────
  // An OPEN listbox/menu popup with ZERO selectable options AND no empty-state
  // hint text ("No models", "No results") shows the user literally nothing to
  // select — the composer model picker with 0 models configured is exactly this
  // (no options, no "No models" text, no configure affordance). Interaction-gated:
  // the popup only exists once opened, driven by an `open-…-select` recipe.
  if (run('H7')) {
    const OPTION_SEL =
      '[role="option"],[role="menuitem"],[role="menuitemradio"],[role="menuitemcheckbox"],[data-slot="select-item"],[cmdk-item]'
    const popups = document.querySelectorAll(
      '[role="listbox"],[role="menu"],[data-slot="select-content"],[data-slot="command-list"]',
    )
    for (const pop of popups) {
      if (isChrome(pop) || inSvg(pop) || !visible(pop)) continue
      if (pop.querySelectorAll(OPTION_SEL).length > 0) continue
      // Any human text in the popup = an empty-state hint ("No models found") → fine.
      if (/[a-z0-9]/i.test(textOf(pop))) continue
      const id = pop.id
      const trigger =
        (id && document.querySelector(`[aria-controls="${id}"]`)) ||
        document.querySelector(
          '[role="combobox"][aria-expanded="true"],[aria-haspopup][aria-expanded="true"]',
        )
      push(
        'H7', selectorFor(trigger || pop),
        `picker dropdown renders nothing: 0 options + no empty-state hint (${selectorFor(pop)}) — the user sees literally nothing to select`,
        { options: 0 },
      )
    }
  }

  // ── I1 z-collision (interactive occluded at center) ──────────────────────
  if (run('I1')) {
    const seen = new Set()
    for (const el of document.querySelectorAll(INTERACTIVE)) {
      if (isChrome(el) || inSvg(el) || !visible(el)) continue
      const r = rectOf(el)
      const cx = r.left + r.width / 2, cy = r.top + r.height / 2
      if (cx < 1 || cy < 1 || cx > VW - 1 || cy > VH - 1) continue
      const top = document.elementFromPoint(cx, cy)
      if (!top || el.contains(top) || top.contains(el) || isChrome(top)) continue
      // coverer must be opaque-ish and clickable (else it's a decorative/pass-through layer)
      const ts = cs(top)
      if (ts.pointerEvents === 'none') continue
      if ((parseColor(ts.backgroundColor)?.a || 0) < 0.5 && !top.querySelector('img,svg')) continue
      const sel = selectorFor(el)
      if (seen.has(sel)) continue
      seen.add(sel)
      push('I1', sel, `interactive <${el.tagName.toLowerCase()}> "${textOf(el).slice(0, 16)}" occluded at center by ${selectorFor(top)} (hit-test miss)`, {})
    }
  }

  // ── I4 modal/sheet within viewport + body scroll lock ────────────────────
  if (run('I4')) {
    const dialogs = document.querySelectorAll('[role="dialog"],[role="alertdialog"],[data-slot="dialog-content"],[data-slot="sheet-content"]')
    for (const d of dialogs) {
      if (isChrome(d) || !visible(d)) continue
      const r = rectOf(d)
      const worst = Math.max(-r.top, -r.left, r.right - VW, r.bottom - VH)
      if (worst > 2 && r.height < VH * 2)
        push('I4', selectorFor(d), `dialog/sheet extends ${Math.round(worst)}px beyond the viewport`, { worst: Math.round(worst) })
    }
    if (dialogs.length) {
      const locked = cs(document.body).overflow === 'hidden' || cs(document.documentElement).overflow === 'hidden' || document.body.hasAttribute('data-scroll-locked')
      if (!locked) push('I4', 'body', `overlay open but body scroll NOT locked`, {}, 'MEDIUM')
    }
  }

  // ── I5 wrong-scroll-axis on a strip ──────────────────────────────────────
  if (run('I5')) {
    const strips = pool.filter(el => {
      const role = el.getAttribute('role')
      const slot = el.getAttribute('data-slot') || ''
      return role === 'tablist' || role === 'toolbar' || /tabs-list|segment|toolbar/.test(slot)
    })
    for (const strip of strips) {
      const s = cs(strip)
      if ((s.overflowY === 'auto' || s.overflowY === 'scroll') && strip.scrollHeight > strip.clientHeight + 4)
        push('I5', selectorFor(strip), `horizontal strip has VERTICAL scroll (scrollHeight ${strip.scrollHeight} > clientHeight ${strip.clientHeight}, overflow-y:${s.overflowY})`, { scrollH: strip.scrollHeight, clientH: strip.clientHeight })
    }
  }

  // ── J6 mixed button variants within a peer action group ──────────────────
  if (run('J6')) {
    const variantOf = btn => {
      const c = clsOf(btn).toLowerCase()
      const dv = btn.getAttribute('data-variant')
      if (dv) return dv
      if (/bg-primary|bg-accent-foreground/.test(c)) return 'default'
      if (/bg-destructive/.test(c)) return 'destructive'
      if (/\bborder\b/.test(c) && /bg-background|bg-transparent/.test(c)) return 'outline'
      if (/\bborder\b/.test(c)) return 'outline'
      if (/hover:bg-accent/.test(c) || /bg-transparent/.test(c)) return 'ghost'
      return 'ghost'
    }
    for (const parent of parentsOf()) {
      const btns = Array.from(parent.children).filter(c => visible(c) && !inSvg(c) && c.matches?.('button,[role="button"]'))
      if (btns.length < 2) continue
      // peers = icon-only, similar size
      const sizes = btns.map(b => { const r = rectOf(b); return Math.round(r.height) })
      const iconOnly = btns.filter(b => !textOf(b) && b.querySelector('svg,img'))
      if (iconOnly.length < 2) continue
      if (Math.max(...sizes) - Math.min(...sizes) > 8) continue
      const variants = new Set(iconOnly.map(variantOf))
      if (variants.size > 1)
        push('J6', selectorFor(parent), `peer icon-only action group mixes button variants: {${[...variants].join(', ')}} — ${iconOnly.map(b => `${b.getAttribute('data-testid') || b.getAttribute('aria-label') || '?'}=${variantOf(b)}`).join(', ')}`, { variants: [...variants].length })
    }
  }

  // ── J7 same-action control side (collect; aggregate in Node) ─────────────
  if (run('J7')) {
    for (const el of document.querySelectorAll(INTERACTIVE)) {
      if (isChrome(el) || inSvg(el) || !visible(el)) continue
      const tid = (el.getAttribute('data-testid') || el.getAttribute('aria-label') || '').toLowerCase()
      const action = actionTokens.find(t => tid.includes(t))
      if (!action) continue
      // find a bounded container (card / panel / toolbar / header)
      const container = el.closest('[data-slot="card"],[role="toolbar"],header,[data-slot="card-header"]') || el.parentElement
      if (!container) continue
      const cr = rectOf(container), r = rectOf(el)
      const mid = cr.left + cr.width / 2
      const side = r.left + r.width / 2 < mid ? 'left' : 'right'
      actionSides.push({ action, side, selector: selectorFor(el), container: selectorFor(container) })
    }
  }

  // ── K1 persistent context inside a scroll container (static form) ────────
  if (run('K1')) {
    const scrollers = pool.filter(el => {
      const s = cs(el)
      return (s.overflowY === 'auto' || s.overflowY === 'scroll') && el.scrollHeight > el.clientHeight + 8
    })
    for (const tid of contextTestids) {
      const el = document.querySelector(`[data-testid="${tid}"]`)
      if (!el || !visible(el) || isChrome(el)) continue
      const scroller = scrollers.find(sc => sc.contains(el) && sc !== el)
      if (scroller) {
        push('K1', `[data-testid="${tid}"]`, `persistent context "${tid}" is a DESCENDANT of scroll container ${selectorFor(scroller)} — scrolls out of view (should be pinned chrome)`, {})
      }
    }
  }

  // ── L1/L2/L3/L4/L5 content-rendering correctness ─────────────────────────
  if (run('L1') || run('L2') || run('L3') || run('L4') || run('L5')) {
    const proseRoots = Array.from(document.querySelectorAll('.prose,[data-slot="markdown"],[class*="markdown"],[data-testid*="markdown"]')).filter(visible)
    const scope = proseRoots.length ? proseRoots : []
    const inScopeText = () => scope.map(s => s.innerText || '').join('\n')
    if (scope.length) {
      const txt = inScopeText()
      if (run('L1')) {
        const rawMath = /\$\$[^$]+\$\$|\\begin\{|\\frac\b|\\sqrt\b/.test(txt)
        const hasKatex = document.querySelector('.katex')
        if (rawMath && !hasKatex)
          push('L1', '.prose', `math NOT rendered: raw TeX ("${(txt.match(/\$\$[^$]+\$\$|\\begin\{[a-z]+\}|\\frac|\\sqrt/) || [''])[0].slice(0, 30)}") present with no .katex output`, {}, 'HIGH')
      }
      if (run('L2')) {
        for (const code of document.querySelectorAll('code,pre')) {
          const t = textOf(code)
          if (/^(graph (TD|LR)|sequenceDiagram|flowchart|gantt|classDiagram)\b/.test(t) && !code.closest('.prose,[class*="markdown"]')?.querySelector('svg'))
            { push('L2', selectorFor(code), `mermaid source ("${t.slice(0, 24)}") did not render to <svg>`, {}); break }
        }
      }
      if (run('L3')) {
        for (const pre of document.querySelectorAll('pre code[class*="language-"],pre[class*="language-"]')) {
          if (!visible(pre)) continue
          const spans = Array.from(pre.querySelectorAll('span')).filter(s => textOf(s))
          const colors = new Set(spans.map(s => cs(s).color))
          if (spans.length === 0 || colors.size <= 1) {
            push('L3', selectorFor(pre), `language-tagged code block has ${spans.length} token spans / ${colors.size} colors — highlighting not applied (single-color plaintext)${preview ? ' [preview-build]' : ' [dev-serve]'}`, { spans: spans.length, colors: colors.size })
            break
          }
        }
      }
      if (run('L4')) {
        // table declared in markdown but no <table>; footnote refs with no target
        // (light fingerprint — presence-only)
        const hasPipeTable = /\|.+\|\n\|[-: |]+\|/.test(txt)
        if (hasPipeTable && !scope.some(s => s.querySelector('table')))
          push('L4', '.prose', `pipe-table syntax present but no <table> rendered`, {})
      }
      if (run('L5')) {
        // a message that rendered totally blank despite having source is L5; we
        // approximate: a prose root that is visible but empty of text AND children
        for (const s of scope) {
          if (!textOf(s) && s.children.length === 0)
            push('L5', selectorFor(s), `markdown container rendered blank (no text, no fallback)`, {})
        }
      }
    }
  }

  // ── C1 status badge ordered BEFORE its label ─────────────────────────────
  // A qualifying badge/tag must FOLLOW the thing it qualifies. A badge that is
  // the FIRST child of its row, carries a status/qualifier word, and is
  // immediately followed by a longer text label reads as "(verified) key"
  // (taxonomy C1, user miss #4). [V] in the taxonomy; this is a narrow geometric
  // proxy (first-child + status vocabulary) to keep the full audit low-noise.
  if (run('C1')) {
    const STATUS_WORDS = /^(verified|unverified|new|beta|alpha|draft|active|inactive|error|failed|pending|done|complete|deprecated|default|required|optional|preview|experimental)$/i
    const isBadge = el =>
      el.getAttribute?.('data-slot') === 'badge' ||
      el.getAttribute?.('data-slot') === 'tag' ||
      /\b(badge|tag|chip|pill)\b/.test(clsOf(el).toLowerCase())
    for (const parent of parentsOf()) {
      const kids = Array.from(parent.children).filter(c => visible(c) && !inSvg(c))
      if (kids.length < 2) continue
      const a = kids[0], b = kids[1]
      if (!isBadge(a) || isBadge(b)) continue
      const ta = textOf(a), tb = textOf(b)
      if (!ta || !tb || !STATUS_WORDS.test(ta)) continue
      const ra = rectOf(a), rb = rectOf(b)
      const sameRow = Math.min(ra.bottom, rb.bottom) - Math.max(ra.top, rb.top) > 4
      if (sameRow && ra.right <= rb.left + 1 && tb.length >= ta.length) {
        push('C1', selectorFor(a),
          `status badge "${ta.slice(0, 16)}" ordered BEFORE its label "${tb.slice(0, 20)}" — a badge should FOLLOW the thing it qualifies`,
          {})
      }
    }
  }

  // ── A10 form control at zero/near-zero size (the "input disappears" class) ─
  if (run('A10')) {
    for (const el of document.querySelectorAll('input:not([type="hidden"]),select,textarea')) {
      if (isChrome(el) || inSvg(el)) continue
      const s = cs(el)
      if (s.display === 'none' || s.visibility === 'hidden' || s.opacity === '0') continue
      const r = rectOf(el)
      const minDim = Math.min(r.width, r.height)
      const maxDim = Math.max(r.width, r.height)
      // near-zero in ONE dimension while the other is real → a control meant to
      // show that collapsed (inline rename form rendering vertical, user miss #16).
      if (minDim < 4 && maxDim >= 6) {
        const parent = el.parentElement
        const pr = parent ? rectOf(parent) : null
        if (pr && pr.width >= 8 && pr.height >= 8) {
          push('A10', selectorFor(el),
            `form control <${el.tagName.toLowerCase()}> rendered ${Math.round(r.width)}×${Math.round(r.height)}px (near-zero ${r.width < r.height ? 'width' : 'height'}) while visible-intent — the "input disappears" class`,
            { w: Math.round(r.width), h: Math.round(r.height) })
        }
      }
    }
  }

  // ── A11 bordered element clipped by an overflow ancestor ─────────────────
  // A3 (protrusion) deliberately SKIPS elements under a clipping ancestor; A11
  // fills that gap — a bordered box whose border-box edge reaches/exceeds a
  // NON-scrolling (overflow:hidden/clip) ancestor's edge has its border painted
  // out (taxonomy A11, user miss #18: tool-call card borders clipped).
  if (run('A11')) {
    for (const el of pool) {
      const s = cs(el)
      if (s.borderStyle === 'none') continue
      const bw = {
        top: parseFloat(s.borderTopWidth) || 0,
        right: parseFloat(s.borderRightWidth) || 0,
        bottom: parseFloat(s.borderBottomWidth) || 0,
        left: parseFloat(s.borderLeftWidth) || 0,
      }
      if (Math.max(bw.top, bw.right, bw.bottom, bw.left) <= 0) continue
      const r = rectOf(el)
      for (const axis of ['x', 'y']) {
        const anc = clippingAncestor(el, axis)
        if (!anc || isChrome(anc)) continue
        // Only overflow:hidden / clip PERMANENTLY cut a border (no way to reveal
        // it). overflow:auto/scroll makes the far edge reachable by scrolling —
        // not a visual defect — AND is silently induced on the perpendicular
        // axis (setting overflow-x:auto computes overflow-y to auto), which was
        // the dominant false positive (scrollable code blocks). So judge ONLY
        // hidden/clip ancestors.
        const ov = axis === 'x' ? cs(anc).overflowX : cs(anc).overflowY
        if (ov !== 'hidden' && ov !== 'clip') continue
        const p = rectOf(anc)
        const sides =
          axis === 'x'
            ? [ { n: 'right', bw: bw.right, cut: r.right - p.right }, { n: 'left', bw: bw.left, cut: p.left - r.left } ]
            : [ { n: 'bottom', bw: bw.bottom, cut: r.bottom - p.bottom }, { n: 'top', bw: bw.top, cut: p.top - r.top } ]
        const hit = sides.find(sd => sd.bw > 0 && sd.cut > 0.5)
        if (hit) {
          push('A11', selectorFor(el),
            `bordered element's ${hit.n} border clipped by overflow-${axis} ancestor ${selectorFor(anc)} (${Math.round(hit.cut)}px past the clip edge)`,
            { side: hit.n, cut: Math.round(hit.cut) })
          break
        }
      }
    }
  }

  // ── A12 cramped double-border (edge-adjacent outline control) ────────────
  if (run('A12')) {
    const borderedAncestor = el => {
      let n = el.parentElement
      while (n && n !== document.body) {
        const s = cs(n)
        if (s.borderStyle !== 'none' &&
          (parseFloat(s.borderTopWidth) || parseFloat(s.borderRightWidth) ||
            parseFloat(s.borderBottomWidth) || parseFloat(s.borderLeftWidth)))
          return n
        n = n.parentElement
      }
      return null
    }
    for (const el of pool) {
      // Target edge-adjacent ACTION controls (the taxonomy A12 case is an outline
      // BUTTON crammed against a container border → should be ghost). A bordered
      // input/select nested in a bordered field is normal design, not a defect.
      if (!el.matches?.('button,[role="button"],a[href]')) continue
      const s = cs(el)
      if (s.borderStyle === 'none') continue
      if (!(parseFloat(s.borderTopWidth) || parseFloat(s.borderRightWidth) ||
        parseFloat(s.borderBottomWidth) || parseFloat(s.borderLeftWidth))) continue
      const anc = borderedAncestor(el)
      if (!anc || isChrome(anc)) continue
      const as = cs(anc)
      const r = rectOf(el), p = rectOf(anc)
      const abl = parseFloat(as.borderLeftWidth) || 0, abr = parseFloat(as.borderRightWidth) || 0
      const abt = parseFloat(as.borderTopWidth) || 0, abb = parseFloat(as.borderBottomWidth) || 0
      const gaps = [
        { n: 'left', g: r.left - (p.left + abl), ab: abl },
        { n: 'right', g: (p.right - abr) - r.right, ab: abr },
        { n: 'top', g: r.top - (p.top + abt), ab: abt },
        { n: 'bottom', g: (p.bottom - abb) - r.bottom, ab: abb },
      ]
      const cramped = gaps.filter(x => x.ab > 0 && x.g >= -1 && x.g < 8)
      if (cramped.length >= 1) {
        push('A12', selectorFor(el),
          `outline ${el.tagName.toLowerCase()} border sits ${cramped.map(c => `${c.n} ${Math.round(c.g)}px`).join(', ')} from the container border (crowded double-border — an edge-adjacent action should be ghost/borderless)`,
          { sides: cramped.length }, 'LOW')
      }
    }
  }

  // ── G9 hover-only controls reserve no space → persistent sibling shifts ───
  if (run('G9')) {
    for (const parent of parentsOf()) {
      const ps = cs(parent)
      if (!ps.display.includes('flex')) continue
      const allKids = Array.from(parent.children)
      const hoverHidden = allKids.filter(c => {
        if (inSvg(c)) return false
        if (cs(c).display !== 'none') return false
        const cl = clsOf(c)
        const sig = /group-hover|hover:|opacity-0/.test(cl) || c.hasAttribute('data-hover-reveal')
        return sig && (c.matches?.('button,[role="button"],a[href]') || !!c.querySelector?.('button,[role="button"]'))
      })
      if (!hoverHidden.length) continue
      const persistent = allKids.filter(c => visible(c) && !inSvg(c))
      if (!persistent.length) continue
      push('G9', selectorFor(parent),
        `${hoverHidden.length} hover-reveal control(s) use display:none (reserve NO layout space) beside ${persistent.length} persistent sibling(s) — the persistent element shifts when hover controls appear; reserve space via visibility/opacity`,
        { hoverHidden: hoverHidden.length })
    }
  }

  // ── H7 empty select/combobox renders nothing ─────────────────────────────
  if (run('H7')) {
    for (const el of document.querySelectorAll('select')) {
      if (isChrome(el) || !visible(el)) continue
      if (el.querySelectorAll('option').length === 0)
        push('H7', selectorFor(el), `<select> has zero <option>s — renders nothing to pick (empty control must say something)`, {})
    }
    for (const el of document.querySelectorAll('[role="combobox"],[data-slot="select-trigger"],[role="listbox"]')) {
      if (isChrome(el) || inSvg(el) || !visible(el)) continue
      if (!textOf(el) && !el.querySelector('svg,img'))
        push('H7', selectorFor(el),
          `${el.getAttribute('role') || el.getAttribute('data-slot')} trigger renders NO text / placeholder / icon — an empty control the user needs must say SOMETHING`,
          {})
    }
  }

  // ── H6 broken image ───────────────────────────────────────────────────────
  if (run('H6')) {
    for (const img of document.querySelectorAll('img')) {
      if (isChrome(img)) continue
      if (img.complete && img.naturalWidth === 0 && img.getAttribute('src'))
        push('H6', selectorFor(img), `broken image (naturalWidth 0): src="${(img.getAttribute('src') || '').slice(0, 50)}"`, {})
    }
  }

  return { findings, actionSides }
}

// ───────────────────────────────────────────────────────────────────────────
function loadAllowlist() {
  if (!fs.existsSync(ALLOWLIST_PATH)) return []
  try {
    const j = JSON.parse(fs.readFileSync(ALLOWLIST_PATH, 'utf8'))
    return Array.isArray(j) ? j : j.entries || []
  } catch {
    return []
  }
}
const isAllowed = (allow, f) =>
  allow.some(
    a =>
      a.class === f.cls &&
      (a.surface === '*' || a.surface === f.surface) &&
      (!a.selector || (f.selector || '').includes(a.selector)) &&
      (!a.viewport || a.viewport === f.viewport),
  )

async function enumerateSurfaces(browser) {
  const p = await browser.newPage({ viewport: { width: 1280, height: 900 } })
  await p.goto(BASE, { waitUntil: 'networkidle' })
  await p.waitForTimeout(2500)
  const pages = []
  for (const s of await p.locator('[data-testid^="gallery-page-"]').all())
    pages.push((await s.getAttribute('data-testid')).replace('gallery-page-', ''))
  const overlays = await p.evaluate(() => window.__GALLERY_OVERLAYS__ || [])
  const deep = await p.evaluate(() => window.__GALLERY_DEEP_STATES__ || [])
  const seeded = await p.evaluate(() => window.__GALLERY_SEEDED__ || [])
  // Interaction recipes ({slug,name}) drive post-mount actions (open a menu,
  // expand a panel) so interaction-gated states get scanned too — else A9's
  // menu-item check / A11 etc. never see e.g. the composer "+" dropdown.
  const interactions = await p.evaluate(() => window.__GALLERY_INTERACTIONS__ || [])
  await p.close()
  const special = new Set([...overlays, ...deep, ...seeded])
  return { pages: pages.filter(x => !special.has(x)), overlays, deep, seeded, interactions }
}

async function main() {
  const browser = await chromium.launch({
    args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu'],
  })
  const { pages, overlays, deep, seeded, interactions } = await enumerateSurfaces(browser)

  // Optional surface filter (substring match) for fast iteration on a few surfaces.
  const surfaceFilter = arg('surfaces', '').split(',').map(s => s.trim()).filter(Boolean)
  const keep = s => !surfaceFilter.length || surfaceFilter.some(f => s.includes(f))
  const cells = []
  for (const s of pages) if (keep(s)) for (const st of PAGE_STATES) cells.push({ surface: s, state: st })
  for (const s of [...seeded, ...deep]) if (keep(s)) cells.push({ surface: s, state: 'seeded' })
  for (const s of overlays) if (keep(s)) cells.push({ surface: s, state: 'open' })
  // Interaction-gated states: one cell per recipe, driven post-mount (open a menu,
  // expand a panel). state='loaded' + the `interact` slug the frame runs on mount.
  for (const it of interactions) if (keep(it.slug)) cells.push({ surface: it.slug, state: 'interact', interact: it.name })

  const jobs = []
  for (const c of cells) for (const vp of VIEWPORTS) jobs.push({ c, vp })
  console.log(
    `geometry-audit${PREVIEW ? ' [preview-build]' : ''}: ${pages.length} pages×${PAGE_STATES.length} + ${seeded.length + deep.length} seeded + ${overlays.length} overlays + ${interactions.length} interactions = ${cells.length} cells × ${VIEWPORTS.length} viewports = ${jobs.length} renders\n`,
  )

  const findings = []
  const actionSides = [] // for J7 cross-surface aggregation
  let done = 0
  const inPageArg = {
    classesArg: CLASS_FILTER,
    contextTestids: CONTEXT_TESTIDS,
    actionTokens: ACTION_SIDE_TOKENS,
    preview: PREVIEW,
  }
  async function runJob({ c, vp }) {
    const p = await browser.newPage({ viewport: { width: vp.width, height: vp.height } })
    const stateParam = c.state === 'seeded' || c.state === 'open' || c.state === 'interact' ? 'loaded' : c.state
    const url = `${BASE}?surface=${c.surface}&state=${stateParam}&theme=light${c.interact ? `&interact=${c.interact}` : ''}`
    try {
      await p.goto(url, { waitUntil: 'domcontentloaded', timeout: 25_000 })
      // Interaction cells: wait for the frame to finish driving the recipe (it
      // stamps body[data-gallery-interact-done]) before scanning the open state.
      if (c.interact) {
        await p.waitForSelector('body[data-gallery-interact-done]', { timeout: 12_000 }).catch(() => {})
      }
      await p.waitForTimeout(c.state === 'error' ? 1100 : 900)
      const { findings: raw, actionSides: sides } = await p.evaluate(inPageGeometry, inPageArg)
      for (const f of raw) {
        findings.push({
          surface: c.surface, state: c.state, viewport: vp.name,
          cls: f.cls, severity: f.severity || CLASS_SEVERITY[f.cls] || 'LOW',
          selector: f.selector, detail: f.detail, nums: f.nums,
        })
      }
      for (const s of sides) actionSides.push({ ...s, surface: c.surface, viewport: vp.name })
    } catch (e) {
      findings.push({ surface: c.surface, state: c.state, viewport: vp.name, cls: 'NAV', severity: 'LOW', selector: null, detail: `nav/eval error: ${(e.message || String(e)).slice(0, 120)}`, nums: {} })
    }
    await p.close()
    if (++done % 40 === 0 || done === jobs.length) console.log(`  … ${done}/${jobs.length} renders`)
  }

  let next = 0
  const worker = async () => { while (next < jobs.length) await runJob(jobs[next++]) }
  await Promise.all(Array.from({ length: Math.min(CONCURRENCY, jobs.length) }, worker))
  await browser.close()

  // J7 cross-surface aggregation: an action token appearing on BOTH sides is a bug
  if (!CLASS_FILTER.length || CLASS_FILTER.includes('J7')) {
    const byAction = {}
    for (const a of actionSides) (byAction[a.action] ??= []).push(a)
    for (const [action, list] of Object.entries(byAction)) {
      const sides = new Set(list.map(a => a.side))
      if (sides.size > 1) {
        // report one finding per minority-side occurrence
        const counts = { left: list.filter(a => a.side === 'left').length, right: list.filter(a => a.side === 'right').length }
        const majority = counts.left >= counts.right ? 'left' : 'right'
        for (const a of list.filter(a => a.side !== majority)) {
          findings.push({
            surface: a.surface, state: 'agg', viewport: a.viewport, cls: 'J7', severity: 'MEDIUM',
            selector: a.selector,
            detail: `"${action}" control on the ${a.side} here but ${majority} in the majority of containers (${counts.left} left / ${counts.right} right) — inconsistent placement`,
            nums: counts,
          })
        }
      }
    }
  }

  const allow = loadAllowlist()
  for (const f of findings) f.allowed = f.severity === 'HIGH' && isAllowed(allow, f)

  writeReports(findings, allow)

  const gatingHigh = findings.filter(f => f.severity === 'HIGH' && !f.allowed)
  if (GATE && gatingHigh.length) {
    console.error(`\n❌ GEOMETRY GATE FAILED — ${gatingHigh.length} non-allow-listed HIGH finding(s).`)
    process.exit(1)
  }
  console.log(GATE ? '\n✅ GEOMETRY GATE PASSED (no un-allow-listed HIGH findings)' : '\n(report-only; pass --gate to fail on un-allow-listed HIGH findings)')
}

function writeReports(findings) {
  fs.mkdirSync(OUT, { recursive: true })
  findings.sort(
    (a, b) =>
      SEVERITY_RANK[b.severity] - SEVERITY_RANK[a.severity] ||
      a.cls.localeCompare(b.cls) ||
      a.surface.localeCompare(b.surface) ||
      a.viewport.localeCompare(b.viewport) ||
      (a.selector || '').localeCompare(b.selector || ''),
  )
  fs.writeFileSync(
    path.join(OUT, 'GEOMETRY_FINDINGS.jsonl'),
    findings.map(f => JSON.stringify(f)).join('\n') + (findings.length ? '\n' : ''),
  )

  const byClass = {}
  for (const f of findings) {
    const c = (byClass[f.cls] ??= { HIGH: 0, MEDIUM: 0, LOW: 0, allowed: 0 })
    c[f.severity]++
    if (f.allowed) c.allowed++
  }
  const bySev = { HIGH: 0, MEDIUM: 0, LOW: 0 }
  for (const f of findings) bySev[f.severity]++
  const allowedCount = findings.filter(f => f.allowed).length
  const gatingHigh = bySev.HIGH - allowedCount

  const md = []
  md.push('# Geometry findings (GENERATED)\n')
  md.push(`> \`node scripts/gallery-geometry-audit.mjs\` (Layer 1 — see \`docs/DEFECT_TAXONOMY.md\`). Deterministic DOM-geometry rules over every gallery surface × state × viewport. Each row cites surface, viewport, taxonomy class, selector, and measured numbers.\n`)
  md.push('## Totals\n')
  md.push('| Severity | Count |\n|---|---|')
  md.push(`| 🔴 HIGH (gating) | ${gatingHigh} |`)
  md.push(`| 🔵 HIGH (allow-listed) | ${allowedCount} |`)
  md.push(`| 🟡 MEDIUM | ${bySev.MEDIUM} |`)
  md.push(`| ⚪ LOW | ${bySev.LOW} |`)
  md.push(`| **Total** | **${findings.length}** |\n`)
  md.push('## By taxonomy class\n')
  md.push('| Class | HIGH | MEDIUM | LOW | allow-listed |\n|---|---|---|---|---|')
  for (const [cls, c] of Object.entries(byClass).sort())
    md.push(`| ${cls} | ${c.HIGH} | ${c.MEDIUM} | ${c.LOW} | ${c.allowed} |`)
  md.push('')

  const gating = findings.filter(f => f.severity === 'HIGH' && !f.allowed)
  md.push(`## Gating HIGH findings (${gating.length})\n`)
  if (!gating.length) md.push('_None — geometry is clean of un-allow-listed HIGH findings._\n')
  else { for (const f of gating) md.push(`- 🔴 **${f.cls}** \`${f.surface}\` [${f.viewport}/${f.state}] — ${f.selector ? `\`${f.selector}\` — ` : ''}${f.detail}`); md.push('') }

  const meds = findings.filter(f => f.severity === 'MEDIUM')
  md.push(`## MEDIUM findings (${meds.length})\n`)
  const byC = {}
  for (const f of meds) (byC[f.cls] ??= []).push(f)
  for (const [cls, list] of Object.entries(byC).sort()) {
    md.push(`### ${cls} (${list.length})\n`)
    for (const f of list.slice(0, 30)) md.push(`- 🟡 \`${f.surface}\` [${f.viewport}] ${f.selector ? `\`${f.selector}\` — ` : ''}${f.detail}`)
    if (list.length > 30) md.push(`- … +${list.length - 30} more (see JSONL)`)
    md.push('')
  }
  fs.writeFileSync(path.join(OUT, 'GEOMETRY_FINDINGS.md'), md.join('\n'))

  console.log(`\n=== geometry-audit: ${findings.length} findings (HIGH ${gatingHigh} gating + ${allowedCount} allow-listed / MEDIUM ${bySev.MEDIUM} / LOW ${bySev.LOW}) ===`)
  console.log('  by class:', Object.entries(byClass).map(([k, c]) => `${k}=${c.HIGH + c.MEDIUM + c.LOW}`).join(' '))
  console.log(`  → ${path.relative(process.cwd(), path.join(OUT, 'GEOMETRY_FINDINGS.md'))}`)
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch(e => {
    console.error(e)
    process.exit(2)
  })
}

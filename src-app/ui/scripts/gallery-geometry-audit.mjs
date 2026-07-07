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
 *   B1 premature wrap/stack  B2 failure-to-wrap  B3 h-overflow
 *   B6 fixed > viewport      B8 mid-word break
 *   C7 indistinguishable roles  C9 icon/label split  C10 icon disproportion
 *   C12 placeholder element
 *   D1 truncated-with-room
 *   G5 tap-target < 44 (mobile)  G7 clipped focus-ring / elevation shadow
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

const SEVERITY_RANK = { HIGH: 3, MEDIUM: 2, LOW: 1 }
/**
 * Per-class default severity. HIGH is reserved for near-zero-false-positive
 * classes so the `--gate` is a meaningful cannot-miss tripwire; the rich review
 * signal lives in MEDIUM/LOW (still fully reported = still "flagged").
 */
const CLASS_SEVERITY = {
  A1: 'HIGH', // real zero-gap between content pills (segmented controls excluded)
  A2: 'MEDIUM',
  A3: 'MEDIUM',
  A4: 'LOW',
  A7: 'LOW',
  A8: 'MEDIUM',
  A9: 'MEDIUM',
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
}

// Context-level testids that must NOT live inside a message/content scroll
// container (K1). Kept in sync with the app's conversation chrome.
const CONTEXT_TESTIDS = [
  'project-header-chip-tag', // the "In project: …" conversation chip
  'conversation-project-chip',
  'conversation-title',
  'conversation-status',
  'chat-mode-indicator',
  'conversation-model-indicator',
]
// Action-control testids whose SIDE (left/right) must be consistent across
// containers (J7). Matched as substrings.
const ACTION_SIDE_TOKENS = [
  'expand', 'collapse', 'fullscreen', 'close', 'copy', 'download',
]

// ───────────────────────────────────────────────────────────────────────────
// The in-page geometry audit. Self-contained (serialized into the page).
// Returns { findings:[{cls,severity?,selector,detail,nums}], actionSides:[{action,side,selector}] }.
// ───────────────────────────────────────────────────────────────────────────
function inPageGeometry({ classesArg, contextTestids, actionTokens, preview }) {
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
  const clippingAncestor = (el, axis) => {
    let n = el.parentElement
    while (n && n !== document.body && n !== document.documentElement) {
      if (clipAxes(n)[axis]) return n
      n = n.parentElement
    }
    return null
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
          return { h: Math.round(r.height), ih: ir ? Math.round(ir.height) : 0, iw: ir ? Math.round(ir.width) : 0 }
        }
        const ms = peers.map(metric)
        for (const field of ['h', 'ih', 'iw']) {
          const vals = ms.map(m => m[field]).filter(v => v > 0)
          if (vals.length < 3) continue
          const freq = {}
          for (const v of vals) freq[v] = (freq[v] || 0) + 1
          const mode = +Object.entries(freq).sort((a, b) => b[1] - a[1])[0][0]
          if (freq[mode] < peers.length / 2) continue // no clear mode → skip
          // element-height legitimately varies by content for CARDS/rows; only
          // judge it for chip/button-sized peers (mode < 48px). Icon box metrics
          // (ih/iw) must match regardless — that's the composer-chip defect.
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
  await p.close()
  const special = new Set([...overlays, ...deep, ...seeded])
  return { pages: pages.filter(x => !special.has(x)), overlays, deep, seeded }
}

async function main() {
  const browser = await chromium.launch({
    args: ['--no-sandbox', '--disable-dev-shm-usage', '--disable-gpu'],
  })
  const { pages, overlays, deep, seeded } = await enumerateSurfaces(browser)

  // Optional surface filter (substring match) for fast iteration on a few surfaces.
  const surfaceFilter = arg('surfaces', '').split(',').map(s => s.trim()).filter(Boolean)
  const keep = s => !surfaceFilter.length || surfaceFilter.some(f => s.includes(f))
  const cells = []
  for (const s of pages) if (keep(s)) for (const st of PAGE_STATES) cells.push({ surface: s, state: st })
  for (const s of [...seeded, ...deep]) if (keep(s)) cells.push({ surface: s, state: 'seeded' })
  for (const s of overlays) if (keep(s)) cells.push({ surface: s, state: 'open' })

  const jobs = []
  for (const c of cells) for (const vp of VIEWPORTS) jobs.push({ c, vp })
  console.log(
    `geometry-audit${PREVIEW ? ' [preview-build]' : ''}: ${pages.length} pages×${PAGE_STATES.length} + ${seeded.length + deep.length} seeded + ${overlays.length} overlays = ${cells.length} cells × ${VIEWPORTS.length} viewports = ${jobs.length} renders\n`,
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
    const url = `${BASE}?surface=${c.surface}&state=${c.state === 'seeded' || c.state === 'open' ? 'loaded' : c.state}&theme=light`
    try {
      await p.goto(url, { waitUntil: 'domcontentloaded', timeout: 25_000 })
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

main().catch(e => {
  console.error(e)
  process.exit(2)
})

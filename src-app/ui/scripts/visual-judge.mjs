#!/usr/bin/env node
/**
 * Layer C — vision-model aesthetic judge (delta-gated, NOT in the test loop).
 *
 * The "does this *look* right" layer. It NEVER runs as part of `playwright test`.
 * Cost is controlled three ways:
 *   1. Layer B gates Layer C — it only judges screenshots that are NEW or CHANGED
 *      (the gallery snapshot PNGs whose content hash isn't already in the verdict
 *      cache). An unchanged screen is never sent to the model.
 *   2. Verdict cache keyed by image hash — identical screenshot → reuse verdict.
 *   3. Section/matrix filters — default to one cell (desktop+light); expand to
 *      dark/mobile/accent only for explicitly-named surfaces.
 *
 * Output is a pre-human triage list of structured findings
 * `{ section, element, issue_type, severity, note }` — humans review the flags,
 * not every pixel. The rubric is derived from
 * `src/components/ui/DESIGN_DIRECTION.md` + the token scale.
 *
 * Usage:
 *   node scripts/visual-judge.mjs --snapshots <dir> [--sections a,b] \
 *        [--only-changed] [--cell desktop/light] [--dry-run] [--out report.json]
 *
 * Triggers (pick per PR; this script is the harness, you wire the trigger):
 *   - diff-gated on PRs: map changed UI files → affected sections → --sections.
 *   - on-demand for a new surface (feature DoD).
 *   - scheduled full-gallery sweep (nightly/weekly): drop --only-changed.
 *   - manual escalation when Layer A/B is ambiguous.
 *
 * Requires ANTHROPIC_API_KEY unless --dry-run. Model via VISUAL_JUDGE_MODEL
 * (default claude-sonnet-4-6 — a cost-sensitive vision tier; raise for hard cases).
 */
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = path.dirname(fileURLToPath(import.meta.url))
const UI_ROOT = path.resolve(HERE, '..')
const DEFAULT_SNAPSHOTS = path.join(UI_ROOT, 'tests/e2e/visual')
const CACHE_PATH = path.join(UI_ROOT, '.visual-judge-cache.json')
const MODEL = process.env.VISUAL_JUDGE_MODEL || 'claude-sonnet-4-6'
const API_URL = 'https://api.anthropic.com/v1/messages'

// ---- args -----------------------------------------------------------------
function parseArgs(argv) {
  const a = {
    snapshots: DEFAULT_SNAPSHOTS,
    sections: null, // null = all
    onlyChanged: false,
    cell: null, // e.g. "desktop/light" → filter filenames containing it
    dryRun: false,
    out: path.join(UI_ROOT, 'visual-judge-report.json'),
    maxImages: Number(process.env.VISUAL_JUDGE_MAX || 40),
  }
  for (let i = 2; i < argv.length; i++) {
    const t = argv[i]
    if (t === '--snapshots') a.snapshots = path.resolve(argv[++i])
    else if (t === '--sections') a.sections = argv[++i].split(',').map(s => s.trim())
    else if (t === '--only-changed') a.onlyChanged = true
    else if (t === '--cell') a.cell = argv[++i]
    else if (t === '--dry-run') a.dryRun = true
    else if (t === '--out') a.out = path.resolve(argv[++i])
    else if (t === '--max') a.maxImages = Number(argv[++i])
    else if (t === '--help' || t === '-h') {
      printHelp()
      process.exit(0)
    }
  }
  return a
}

function printHelp() {
  console.log(
    fs.readFileSync(fileURLToPath(import.meta.url), 'utf-8')
      .split('\n')
      .filter(l => l.startsWith(' *') || l.startsWith('/**'))
      .map(l => l.replace(/^ \*?/, '').replace(/^\/\*\*/, ''))
      .join('\n'),
  )
}

// ---- rubric (derived from DESIGN_DIRECTION.md + token scale) ---------------
const RUBRIC = `You are a meticulous UI design reviewer for "ziee" — a research workbench for
life scientists. The design identity ("bench notebook") is refined & precise:
differentiation rests on typography and structure, NOT a loud color. Judge each
screenshot of an isolated component-gallery SECTION against these enforceable rules:

LAYOUT & SPACING
- Spacing (padding/margin/gap) must read as a consistent 2px/4px rhythm. Flag
  cramped or uneven gaps, lopsided padding, elements that touch or collide.
- No element overflows or is clipped; no accidental horizontal scroll.
- A non-full-width button must NOT span the whole row ("too big").
- Aligned elements must share edges; flag ragged left/right edges in a group.

SIZING & HIERARCHY
- Control heights within a row should match (inputs/buttons/selects align).
- Type scale should be deliberate: headings clearly above body; nothing looks
  like a default/untuned size. Flag a control that's oversized vs its siblings.

COLOR & CONTRAST (tokens only)
- The accent is the single brand hue (primary/focus/selected). Flag any color
  that fights it or looks like a raw palette hue rather than a tuned token.
- Text must meet WCAG AA against its background in BOTH light and dark. Flag
  low-contrast text, muted text that's too faint, disabled states indistinguishable.

STATE & POLISH
- Disabled/loading/invalid/selected states must be visually distinct + legible.
- Focus rings visible and on-brand. Motion is minimal; nothing looks broken.

Be specific and conservative: only report real, visible problems. Subjective
brand taste is for humans — you catch alignment/spacing/sizing/overflow/hierarchy/
contrast defects.

Return ONLY valid JSON: an array of findings, each:
{ "element": "<which case/control>", "issue_type":
"alignment|spacing|sizing|overflow|hierarchy|contrast", "severity":
"low|medium|high", "note": "<one concrete sentence>" }
Return [] if the section looks correct.`

// ---- image selection (delta-gate) -----------------------------------------
function walk(dir, acc = []) {
  if (!fs.existsSync(dir)) return acc
  for (const e of fs.readdirSync(dir)) {
    const full = path.join(dir, e)
    const st = fs.statSync(full)
    if (st.isDirectory()) walk(full, acc)
    else if (/\.png$/.test(e) && !/-(actual|diff)\.png$/.test(e)) acc.push(full)
  }
  return acc
}

const sha256 = buf => createHash('sha256').update(buf).digest('hex')

function loadCache() {
  try {
    return JSON.parse(fs.readFileSync(CACHE_PATH, 'utf-8'))
  } catch {
    return {}
  }
}

function sectionOf(file) {
  // baseline name: gallery-section-<id>-<vp>-<theme>-<accent>.png
  const m = path.basename(file).match(/^(gallery-section-[a-z0-9-]+?)-(mobile|tablet|desktop)-/i)
  return m ? m[1] : path.basename(file).replace(/\.png$/, '')
}

function selectImages(args, cache) {
  let files = walk(args.snapshots)
  if (args.cell) files = files.filter(f => path.basename(f).includes(args.cell.replace('/', '-')))
  if (args.sections) {
    files = files.filter(f => args.sections.some(s => sectionOf(f).includes(s)))
  }
  const selected = []
  for (const f of files) {
    const hash = sha256(fs.readFileSync(f))
    const cached = cache[hash]
    if (args.onlyChanged && cached) continue // B-gated: skip unchanged (cached) images
    selected.push({ file, hash, cached })
  }
  return selected.slice(0, args.maxImages)
}

// ---- model call -----------------------------------------------------------
async function judgeImage(file) {
  const b64 = fs.readFileSync(file).toString('base64')
  const body = {
    model: MODEL,
    max_tokens: 1500,
    messages: [
      {
        role: 'user',
        content: [
          { type: 'text', text: `${RUBRIC}\n\nSection file: ${path.basename(file)}` },
          {
            type: 'image',
            source: { type: 'base64', media_type: 'image/png', data: b64 },
          },
        ],
      },
    ],
  }
  const res = await fetch(API_URL, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-api-key': process.env.ANTHROPIC_API_KEY,
      'anthropic-version': '2023-06-01',
    },
    body: JSON.stringify(body),
  })
  if (!res.ok) throw new Error(`Anthropic API ${res.status}: ${await res.text()}`)
  const json = await res.json()
  const text = (json.content || []).filter(c => c.type === 'text').map(c => c.text).join('')
  return parseFindings(text)
}

function parseFindings(text) {
  const start = text.indexOf('[')
  const end = text.lastIndexOf(']')
  if (start === -1 || end === -1) return []
  try {
    const arr = JSON.parse(text.slice(start, end + 1))
    return Array.isArray(arr) ? arr : []
  } catch {
    return []
  }
}

// ---- main -----------------------------------------------------------------
async function main() {
  const args = parseArgs(process.argv)
  const cache = loadCache()
  const targets = selectImages(args, cache)

  console.log(
    `visual-judge: ${targets.length} image(s) to judge ` +
      `(${args.onlyChanged ? 'delta-gated' : 'full'}; model ${MODEL}; ` +
      `${args.dryRun ? 'DRY RUN' : 'live'})`,
  )
  if (!targets.length) {
    console.log('Nothing to judge — all selected images are cached/unchanged.')
    return
  }

  if (args.dryRun) {
    for (const t of targets) {
      console.log(`  would judge: ${path.relative(UI_ROOT, t.file)} (${t.hash.slice(0, 12)})`)
    }
    return
  }

  if (!process.env.ANTHROPIC_API_KEY) {
    console.error('ANTHROPIC_API_KEY is required (or use --dry-run).')
    process.exit(2)
  }

  const report = { model: MODEL, generatedAt: new Date().toISOString(), findings: [] }
  for (const t of targets) {
    let findings = []
    if (t.cached?.findings) {
      findings = t.cached.findings // exact hash reuse
    } else {
      try {
        findings = await judgeImage(t.file)
      } catch (e) {
        console.error(`  ! ${path.basename(t.file)}: ${e.message}`)
        continue
      }
      cache[t.hash] = { file: path.basename(t.file), findings, at: new Date().toISOString() }
    }
    const section = sectionOf(t.file)
    for (const f of findings) report.findings.push({ section, image: path.basename(t.file), ...f })
    console.log(`  ${findings.length ? '⚠' : '✓'} ${path.basename(t.file)} — ${findings.length} finding(s)`)
  }

  fs.writeFileSync(CACHE_PATH, JSON.stringify(cache, null, 2))
  fs.writeFileSync(args.out, JSON.stringify(report, null, 2))
  console.log(
    `\n${report.findings.length} finding(s) across ${targets.length} image(s) → ${path.relative(UI_ROOT, args.out)}`,
  )
}

main().catch(e => {
  console.error(e)
  process.exit(1)
})

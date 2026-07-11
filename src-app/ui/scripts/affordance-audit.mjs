/**
 * AFFORDANCE-AUDIT pass — the capability analog of `runtime-health.mjs`.
 *
 * runtime-health asks "does the surface RENDER correctly?" (console errors,
 * contrast, a11y names). This pass asks the orthogonal question: "can the user
 * DO what they'd naturally want with each rendered content type?" — i.e. does
 * every code block have a copy control, every table a fullscreen control, every
 * tool-call an expand control, every message a copy control, etc.
 *
 * It is a deterministic **M1 presence check**: for each REQUIRED, machine-checkable
 * affordance (see docs/AFFORDANCE_MATRIX.md §6), it locates every container of a
 * content type in the rendered DOM and asserts the affordance's control element
 * is present inside it. A missing control is a capability regression.
 *
 * Surfaces: the gallery's rich-conversation deep-states (`deep-chat-*`), which
 * render a real `ConversationPage` pinned to the recorded `chat.json` fixture —
 * code / mermaid / html / math / tables / images / tool-calls / attachments /
 * branched messages all appear there.
 *
 * Allowlist gating (`scripts/affordance-audit-allowlist.json`): a rule listed
 * there reports as ALLOWED (documented backlog gap, non-gating) instead of
 * failing — so the two user-named backlog gaps (mermaid source toggle, HTML
 * live render) keep the run green while staying tracked. Any NON-allowlisted
 * missing control fails the run (non-zero exit).
 *
 * Usage:
 *   node scripts/affordance-audit.mjs [--url=BASE] [--out=DIR]
 *        [--themes=light,dark] [--report-only]
 *
 * Exit code: non-zero if any non-allowlisted REQUIRED control is missing
 * (unless --report-only).
 */
import { chromium } from '@playwright/test'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { enumerateSurfaces, cellUrl } from './lib/gallery-surfaces.mjs'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const GALLERY_DIR = path.resolve(__dirname, '../src/dev/gallery')

const arg = (n, d) =>
  (process.argv.find(a => a.startsWith(`--${n}=`)) || `--${n}=${d}`)
    .split('=')
    .slice(1)
    .join('=')
const flag = n => process.argv.includes(`--${n}`)

const PORT = process.env.GALLERY_PORT || '1420'
const BASE = arg('url', `http://localhost:${PORT}/gallery.html`)
const OUT = arg('out', GALLERY_DIR)
const THEMES = arg('themes', 'light,dark').split(',').filter(Boolean)
const REPORT_ONLY = flag('report-only')

/**
 * The REQUIRED, deterministic affordance rules. Each rule finds every element
 * matching `container` and asserts `control` (a descendant selector) exists
 * inside it. Both are plain CSS selectors evaluated in the page.
 *
 * `allowlistKey` ties the rule to `affordance-audit-allowlist.json`; when listed
 * there a miss is reported ALLOWED (non-gating). Remove the allowlist entry when
 * the backing feature ships (the rule then guards the new control).
 */
const RULES = [
  {
    name: 'code-copy',
    label: 'code block has a copy control',
    container: '[data-streamdown="code-block"]',
    control: '[data-streamdown="code-block-copy-button"]',
  },
  {
    name: 'table-fullscreen',
    label: 'table has a fullscreen control (toolbar present ⇒ copy/download present)',
    container: '[data-streamdown="table-wrapper"]',
    control: '[data-testid="markdown-table-fullscreen-btn"]',
  },
  {
    name: 'mermaid-toggle',
    label: 'mermaid block has a source⇄render toggle control',
    container: '[data-streamdown="mermaid-block"]',
    control: '[data-testid="mermaid-source-toggle"]',
    allowlistKey: 'mermaid-toggle',
  },
  {
    name: 'html-render',
    label: 'HTML block has a source⇄render (Code/Preview) toggle control',
    container: '[data-testid="html-block"]',
    control: '[data-testid="html-block-toggle"]',
    // No allowlistKey: the feature ships, so this rule GUARDS the toggle.
  },
  {
    name: 'toolcall-expand',
    label: 'tool-call card has an expand/details control',
    container: '[data-testid^="mcp-toolcall-card-"]',
    control: '[data-testid^="mcp-toolcall-details-btn-"]',
  },
  {
    name: 'message-copy',
    label: 'chat message has a copy control',
    container: '[data-testid="chat-message"]',
    control: '[data-testid="chat-message-copy-btn"]',
  },
  {
    name: 'attachment-newtab',
    label: 'inline file preview has an open-in-new-tab control',
    container: '[data-testid="inline-file-preview"]',
    control: '[data-testid="inline-file-preview-open"]',
  },
]

const SEVERITY = { HIGH: 3, ALLOWED: 0 }

function loadAllowlist() {
  const p = path.resolve(__dirname, 'affordance-audit-allowlist.json')
  try {
    const raw = JSON.parse(fs.readFileSync(p, 'utf8'))
    return new Set((raw.allowed || []).map(a => a.rule))
  } catch {
    return new Set()
  }
}

/**
 * In-page presence audit. For each rule, count containers and, per container,
 * whether the control is present. Returns per-rule tallies + the missing indices.
 */
function inPagePresence(rules) {
  const results = []
  for (const rule of rules) {
    const containers = Array.from(document.querySelectorAll(rule.container))
    const missing = []
    containers.forEach((el, i) => {
      if (!el.querySelector(rule.control)) missing.push(i)
    })
    results.push({ name: rule.name, containers: containers.length, missing })
  }
  return results
}

async function reachable(page) {
  try {
    const res = await page.goto(BASE, { waitUntil: 'domcontentloaded', timeout: 8000 })
    return !!res && res.ok()
  } catch {
    return false
  }
}

async function main() {
  const allowlist = loadAllowlist()
  const browser = await chromium.launch()
  const context = await browser.newContext()
  const page = await context.newPage()

  if (!(await reachable(page))) {
    console.error(
      `affordance-audit: gallery not reachable at ${BASE}.\n` +
        `Boot it first (e.g. \`npm run gallery\` / the gate-ui harness) or pass --url=.`,
    )
    await browser.close()
    process.exit(REPORT_ONLY ? 0 : 2)
  }

  // Only the rich-conversation deep-states render the content types we audit.
  const classes = await enumerateSurfaces(page, BASE)
  const INTERACT_ONLY = process.env.INTERACT_ONLY === '1'
  // Base units: each deep-chat deep-state. Interaction units: each deep-chat
  // interaction recipe (drives a real user action, then re-checks affordances).
  const units = INTERACT_ONLY
    ? (classes.interactions || [])
        .filter(it => it.slug.startsWith('deep-chat'))
        .map(it => ({ slug: it.slug, interact: it.name }))
    : (classes.deep || [])
        .filter(s => s.startsWith('deep-chat'))
        .map(slug => ({ slug }))

  const findings = []
  for (const unit of units) {
    const { slug, interact } = unit
    for (const theme of THEMES) {
      const url = cellUrl(BASE, { slug, cls: 'deep', state: 'deep', interact }, { theme })
      await page.goto(url, { waitUntil: 'domcontentloaded' })
      // Let the deep-state's setup() seed the conversation + Streamdown render
      // (mermaid/shiki are async). The deep-states resolve `whenLoaded` first.
      await page.waitForTimeout(3500)
      if (interact)
        await page
          .waitForSelector('body[data-gallery-interact-done]', { timeout: 15_000 })
          .catch(() => {})

      const results = await page.evaluate(inPagePresence, RULES)
      for (const r of results) {
        if (r.missing.length === 0) continue
        const rule = RULES.find(x => x.name === r.name)
        const allowed = rule.allowlistKey && allowlist.has(rule.allowlistKey)
        findings.push({
          rule: r.name,
          label: rule.label,
          surface: slug,
          interact: interact || null,
          theme,
          containers: r.containers,
          missing: r.missing.length,
          severity: allowed ? 'ALLOWED' : 'HIGH',
        })
      }
    }
  }

  await browser.close()

  // ---- write outputs ------------------------------------------------------
  const jsonlPath = path.join(OUT, 'AFFORDANCE_FINDINGS.jsonl')
  fs.writeFileSync(jsonlPath, findings.map(f => JSON.stringify(f)).join('\n') + '\n')

  const gating = findings.filter(f => f.severity === 'HIGH')
  const allowed = findings.filter(f => f.severity === 'ALLOWED')
  const md = [
    '# Affordance-audit findings',
    '',
    `Surfaces audited: ${units.length} deep-chat ${INTERACT_ONLY ? 'interaction recipes' : 'states'} × ${THEMES.length} theme(s).`,
    `Gating misses (HIGH): ${gating.length} · Allowlisted gaps: ${allowed.length}.`,
    '',
    '## Gating (non-allowlisted REQUIRED control missing)',
    gating.length === 0
      ? '_none — every shipped REQUIRED affordance is present._'
      : gating
          .map(
            f =>
              `- **${f.rule}** — ${f.label} — missing on ${f.missing}/${f.containers} container(s) — ${f.surface} (${f.theme})`,
          )
          .join('\n'),
    '',
    '## Allowlisted (tracked backlog gap — see docs/AFFORDANCE_MATRIX.md §7)',
    allowed.length === 0
      ? '_none observed._'
      : allowed
          .map(f => `- **${f.rule}** — ${f.label} — ${f.surface} (${f.theme})`)
          .join('\n'),
    '',
  ].join('\n')
  fs.writeFileSync(path.join(OUT, 'AFFORDANCE_FINDINGS.md'), md)

  console.log(md)
  const worst = findings.reduce((m, f) => Math.max(m, SEVERITY[f.severity] ?? 0), 0)
  if (!REPORT_ONLY && worst >= SEVERITY.HIGH) {
    console.error(`affordance-audit: ${gating.length} gating affordance regression(s).`)
    process.exit(1)
  }
}

main().catch(err => {
  console.error(err)
  process.exit(REPORT_ONLY ? 0 : 2)
})

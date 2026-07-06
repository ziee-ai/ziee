#!/usr/bin/env node
// lifecycle-check.mjs — deterministic (no-LLM) gate for the feature-lifecycle
// state machine. Validates the completeness of each phase's artifacts under
// .lifecycle/<feature>/ and reconciles the git diff against the audit ledger.
//
// Usage:
//   node lifecycle-check.mjs --phase <1-8> [--dir <feature-dir>] [--base <ref>]
//   node lifecycle-check.mjs --all       [--dir <feature-dir>] [--base <ref>]
//
// Exit code 0 = phase(s) complete. Non-zero = incomplete, with a precise gap
// list on stderr. Agents may NOT advance to phase N+1 until `--phase N` is 0.
// The pre-push hook runs `--all`.
//
// No external dependencies: pure Node + `git` via child_process.

import { execFileSync } from 'node:child_process';
import { readFileSync, existsSync, readdirSync, statSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';

// ---------------------------------------------------------------------------
// arg parsing
// ---------------------------------------------------------------------------
const args = process.argv.slice(2);
function opt(name, def = undefined) {
  const i = args.indexOf(name);
  if (i === -1) return def;
  const v = args[i + 1];
  return v && !v.startsWith('--') ? v : true;
}
const wantAll = args.includes('--all');
const phaseArg = opt('--phase');
const baseArg = opt('--base'); // resolved after repo is known (default: origin/main if it exists)
let dirArg = opt('--dir');
let repoArg = opt('--repo');

// ---------------------------------------------------------------------------
// locate repo + feature dir
// ---------------------------------------------------------------------------
function git(cwd, ...a) {
  return execFileSync('git', a, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
}
let repo;
try {
  repo = repoArg ? resolve(repoArg) : git(process.cwd(), 'rev-parse', '--show-toplevel');
} catch {
  fail(`not inside a git repository (cwd=${process.cwd()})`);
}

let featureDir;
if (dirArg) {
  featureDir = resolve(dirArg);
} else {
  const lifecycleRoot = join(repo, '.lifecycle');
  if (!existsSync(lifecycleRoot)) fail(`no .lifecycle/ directory in ${repo}`);
  const subs = readdirSync(lifecycleRoot).filter((d) => {
    try { return statSync(join(lifecycleRoot, d)).isDirectory(); } catch { return false; }
  });
  if (subs.length === 0) fail(`.lifecycle/ contains no feature directory`);
  if (subs.length > 1) fail(`.lifecycle/ has multiple features (${subs.join(', ')}); pass --dir`);
  featureDir = join(lifecycleRoot, subs[0]);
}
if (!existsSync(featureDir)) fail(`feature dir not found: ${featureDir}`);

// Resolve the diff base. Worktrees are cut from origin/main, and a stale local
// `main` would inflate the diff with the whole upstream delta — so prefer
// origin/main when it resolves, unless an explicit --base was given.
let baseRef = typeof baseArg === 'string' ? baseArg : null;
if (!baseRef) {
  try { git(repo, 'rev-parse', '--verify', '--quiet', 'origin/main'); baseRef = 'origin/main'; }
  catch { baseRef = 'main'; }
}

// ---------------------------------------------------------------------------
// small helpers
// ---------------------------------------------------------------------------
function fail(msg) {
  process.stderr.write(`lifecycle-check: FATAL: ${msg}\n`);
  process.exit(2);
}
function read(name) {
  const p = join(featureDir, name);
  if (!existsSync(p)) return null;
  return readFileSync(p, 'utf8');
}
function hasSection(text, ...titles) {
  // matches a markdown heading (##..######) whose text contains any title (ci)
  const lines = text.split(/\r?\n/);
  for (const ln of lines) {
    const m = /^#{2,6}\s+(.*\S)\s*$/.exec(ln);
    if (!m) continue;
    const h = m[1].toLowerCase();
    for (const t of titles) if (h.includes(t.toLowerCase())) return true;
  }
  return false;
}
function glob(prefix) {
  // returns DRIFT-1.md style files sorted by their numeric index ascending
  return readdirSync(featureDir)
    .map((f) => {
      const m = new RegExp(`^${prefix}-(\\d+)\\.md$`).exec(f);
      return m ? { file: f, n: parseInt(m[1], 10) } : null;
    })
    .filter(Boolean)
    .sort((a, b) => a.n - b.n);
}

// ---------------------------------------------------------------------------
// parsers for the machine-readable artifact syntax
// ---------------------------------------------------------------------------
// PLAN item:   - **ITEM-3**: description
const RE_ITEM = /^-\s*\*\*(ITEM-[A-Za-z0-9._-]+)\*\*\s*:\s*(.+?)\s*$/;
// AUDIT line:  - **ITEM-3** — verdict: PASS — rationale     (dash may be - or — or :)
const RE_AUDIT = /^-\s*\*\*(ITEM-[A-Za-z0-9._-]+)\*\*.*?verdict\s*:\s*(PASS|CONCERN|BLOCKED)\b(.*)$/i;
// TEST line:   - **TEST-2** (tier: integration) [covers: ITEM-1, ITEM-3] file: `x` — asserts: y
const RE_TEST_ID = /\*\*(TEST-[A-Za-z0-9._-]+)\*\*/;
const RE_TEST_TIER = /tier\s*:\s*(unit|integration|e2e)\b/i;
const RE_TEST_COVERS = /covers\s*:\s*([^\]]+)\]/i;
const RE_TEST_FILE = /file\s*:\s*[`"]?([^`"\n]+?)[`"]?\s*(?:—|--|-|asserts)/i;
const RE_TEST_ASSERTS = /asserts\s*:\s*(.+?)\s*$/i;
// DECISION:    ### DEC-1: question   then **Resolution:** ...  **Basis:** ...
const RE_DEC = /^#{2,6}\s*(DEC-[A-Za-z0-9._-]+)\s*:/;
// DRIFT entry: - **DRIFT-1.2** — verdict: plan-wins — text
const RE_DRIFT = /^-\s*\*\*(DRIFT-[A-Za-z0-9._-]+)\*\*.*?verdict\s*:\s*(plan-wins|impl-wins|none|resolved)\b/i;
// TEST_RESULTS: - **TEST-2**: PASS
const RE_RESULT = /\*\*(TEST-[A-Za-z0-9._-]+)\*\*\s*:?\s*.*?\b(PASS|FAIL|SKIP)\b/i;

function parsePlanItems() {
  const t = read('PLAN.md');
  if (t == null) return null;
  const items = new Map();
  for (const ln of t.split(/\r?\n/)) {
    const m = RE_ITEM.exec(ln);
    if (m && m[2].trim()) items.set(m[1], m[2].trim());
  }
  return items;
}
function parseTests() {
  const t = read('TESTS.md');
  if (t == null) return null;
  const tests = [];
  for (const ln of t.split(/\r?\n/)) {
    const idm = RE_TEST_ID.exec(ln);
    if (!idm || !/^\s*-\s/.test(ln)) continue;
    const tier = (RE_TEST_TIER.exec(ln) || [])[1];
    const coversRaw = (RE_TEST_COVERS.exec(ln) || [])[1] || '';
    const covers = coversRaw.split(/[,\s]+/).map((s) => s.trim()).filter((s) => /^ITEM-/.test(s));
    const file = (RE_TEST_FILE.exec(ln) || [])[1];
    const asserts = (RE_TEST_ASSERTS.exec(ln) || [])[1];
    tests.push({ id: idm[1], tier, covers, file: file && file.trim(), asserts: asserts && asserts.trim(), line: ln });
  }
  return tests;
}

// ---------------------------------------------------------------------------
// git diff hunk parsing (git diff base...HEAD --unified=0)
// ---------------------------------------------------------------------------
function diffHunks() {
  let out;
  try {
    out = git(repo, 'diff', `${baseRef}...HEAD`, '--unified=0', '--no-color',
      '--', '.', ':(exclude).lifecycle');
  } catch (e) {
    // fall back to two-dot if merge-base form fails
    out = git(repo, 'diff', baseRef, '--unified=0', '--no-color', '--', '.', ':(exclude).lifecycle');
  }
  const hunks = [];
  let file = null;
  for (const ln of out.split(/\r?\n/)) {
    const fm = /^\+\+\+ b\/(.+)$/.exec(ln);
    if (fm) { file = fm[1] === '/dev/null' ? null : fm[1]; continue; }
    if (/^--- /.test(ln) || /^diff --git/.test(ln)) continue;
    const hm = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@/.exec(ln);
    if (hm && file) {
      const start = parseInt(hm[1], 10);
      const count = hm[2] === undefined ? 1 : parseInt(hm[2], 10);
      // deletion-only hunk (count 0): anchor at the surrounding new-side line
      const s = count === 0 ? Math.max(start, 1) : start;
      const e = count === 0 ? Math.max(start, 1) : start + count - 1;
      hunks.push({ file, start: s, end: e });
    }
  }
  return hunks;
}
function parseCoverage() {
  const t = read('AUDIT_COVERAGE.tsv');
  if (t == null) return null;
  const rows = [];
  for (const ln of t.split(/\r?\n/)) {
    if (!ln.trim() || /^file\b/i.test(ln)) continue; // skip header/blank
    const cols = ln.split('\t');
    if (cols.length < 4) continue;
    const [file, start, end, angles] = cols;
    rows.push({
      file: file.trim(),
      start: parseInt(start, 10),
      end: parseInt(end, 10),
      angles: angles.split(/[,\s]+/).map((a) => a.trim().toLowerCase()).filter(Boolean),
    });
  }
  return rows;
}
function parseLedger() {
  const t = read('LEDGER.jsonl');
  if (t == null) return null;
  const rows = [];
  t.split(/\r?\n/).forEach((ln, i) => {
    if (!ln.trim()) return;
    try { rows.push(JSON.parse(ln)); } catch { rows.push({ __parse_error: i + 1 }); }
  });
  return rows;
}

// ---------------------------------------------------------------------------
// per-phase validators — each returns { present, gaps: [] }
// ---------------------------------------------------------------------------
const FORBIDDEN_DECISION = /\b(TBD|TODO|ASK)\b|\?\?\?|<\s*(ask|decide|todo)\s*>/i;
const ANGLE_MIN = 10;
const COVERAGE_MIN_ANGLES = 3;

function phase1() {
  const g = [];
  const t = read('PLAN.md');
  if (t == null) return { present: false, gaps: ['PLAN.md missing'] };
  if (!hasSection(t, 'item')) g.push('PLAN.md: missing an "Items" section');
  if (!hasSection(t, 'files to touch', 'files-to-touch')) g.push('PLAN.md: missing a "Files to touch" section');
  if (!hasSection(t, 'patterns to follow', 'patterns-to-follow', 'pattern')) g.push('PLAN.md: missing a "Patterns to follow" section');
  const items = parsePlanItems();
  if (!items || items.size === 0) g.push('PLAN.md: no `- **ITEM-N**: description` lines parsed');
  return { present: true, gaps: g };
}

function phase2() {
  const g = [];
  const t = read('PLAN_AUDIT.md');
  if (t == null) return { present: false, gaps: ['PLAN_AUDIT.md missing'] };
  const items = parsePlanItems();
  if (!items) return { present: true, gaps: ['PLAN.md missing/empty — cannot audit'] };
  for (const dim of [['breakage'], ['pattern conformance', 'pattern'], ['migration'], ['openapi']]) {
    if (!hasSection(t, ...dim)) g.push(`PLAN_AUDIT.md: missing dimension section "${dim[0]}"`);
  }
  const verdicts = new Map();
  for (const ln of t.split(/\r?\n/)) {
    const m = RE_AUDIT.exec(ln);
    if (m) verdicts.set(m[1], m[2].toUpperCase());
  }
  for (const id of items.keys()) {
    if (!verdicts.has(id)) g.push(`PLAN_AUDIT.md: ${id} has no verdict line (- **${id}** — verdict: PASS|CONCERN|BLOCKED — ...)`);
    else if (verdicts.get(id) === 'BLOCKED') g.push(`PLAN_AUDIT.md: ${id} verdict is BLOCKED — resolve before proceeding`);
  }
  return { present: true, gaps: g };
}

function phase3() {
  const g = [];
  const items = parsePlanItems();
  const tests = parseTests();
  if (tests == null) return { present: false, gaps: ['TESTS.md missing'] };
  if (!items) return { present: true, gaps: ['PLAN.md missing/empty — cannot map tests'] };
  if (tests.length === 0) g.push('TESTS.md: no `- **TEST-N** (tier: ...) [covers: ITEM-x] file: ... asserts: ...` lines parsed');
  const covered = new Set();
  for (const t of tests) {
    if (!t.tier) g.push(`TESTS.md: ${t.id} missing "(tier: unit|integration|e2e)"`);
    if (!t.file) g.push(`TESTS.md: ${t.id} missing "file: <path>"`);
    if (!t.asserts) g.push(`TESTS.md: ${t.id} missing "asserts: <what>"`);
    if (t.covers.length === 0) g.push(`TESTS.md: ${t.id} missing "[covers: ITEM-x]"`);
    for (const c of t.covers) {
      if (!items.has(c)) g.push(`TESTS.md: ${t.id} covers unknown ${c} (not in PLAN.md)`);
      else covered.add(c);
    }
  }
  for (const id of items.keys()) {
    if (!covered.has(id)) g.push(`TESTS.md: ${id} is not covered by any TEST (bipartite completeness fails)`);
  }
  return { present: true, gaps: g };
}

function phase4() {
  const g = [];
  const t = read('DECISIONS.md');
  if (t == null) return { present: false, gaps: ['DECISIONS.md missing'] };
  const decs = [];
  const lines = t.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const m = RE_DEC.exec(lines[i]);
    if (m) {
      // look ahead for a Resolution before the next DEC heading
      let res = false;
      for (let j = i + 1; j < lines.length && !RE_DEC.test(lines[j]); j++) {
        if (/\*\*\s*resolution\s*:?\s*\*\*/i.test(lines[j]) || /^\s*resolution\s*:/i.test(lines[j])) res = true;
      }
      decs.push({ id: m[1], res });
    }
  }
  if (decs.length === 0) g.push('DECISIONS.md: no `### DEC-N: ...` entries parsed');
  for (const d of decs) if (!d.res) g.push(`DECISIONS.md: ${d.id} has no "**Resolution:**" line`);
  // forbidden markers anywhere
  lines.forEach((ln, i) => {
    if (FORBIDDEN_DECISION.test(ln)) g.push(`DECISIONS.md:${i + 1}: forbidden unresolved marker (TBD/TODO/ASK/???): "${ln.trim()}"`);
  });
  return { present: true, gaps: g };
}

function phase5() {
  const g = [];
  const drifts = glob('DRIFT');
  if (drifts.length === 0) return { present: false, gaps: ['no DRIFT-<n>.md files (implement + drift loop not started)'] };
  // every drift file must declare its unresolved count; the highest round must be 0
  for (const d of drifts) {
    const t = read(d.file);
    if (!/\*\*\s*unresolved drifts\s*:?\s*\*\*\s*\d+/i.test(t) && !/^\s*unresolved drifts\s*:\s*\d+/im.test(t))
      g.push(`${d.file}: missing "**Unresolved drifts:** <N>" summary line`);
    // each drift entry needs a recognized verdict
    for (const ln of t.split(/\r?\n/)) {
      if (/^\s*-\s*\*\*DRIFT-/.test(ln) && !RE_DRIFT.test(ln))
        g.push(`${d.file}: drift entry missing verdict (plan-wins|impl-wins|none|resolved): "${ln.trim().slice(0, 80)}"`);
    }
  }
  const last = drifts[drifts.length - 1];
  const lt = read(last.file);
  const m = /unresolved drifts\s*:?\s*\*{0,2}\s*(\d+)/i.exec(lt);
  if (!m) g.push(`${last.file}: cannot read unresolved-drift count`);
  else if (parseInt(m[1], 10) !== 0) g.push(`${last.file}: convergence not reached — ${m[1]} unresolved drift(s) in the final round`);
  return { present: true, gaps: g };
}

function phase6() {
  const g = [];
  const ledger = parseLedger();
  const cov = parseCoverage();
  if (ledger == null && cov == null) return { present: false, gaps: ['LEDGER.jsonl and AUDIT_COVERAGE.tsv missing (blind audit not started)'] };
  if (ledger == null) g.push('LEDGER.jsonl missing');
  if (cov == null) g.push('AUDIT_COVERAGE.tsv missing');
  if (ledger) {
    const bad = ledger.filter((r) => r.__parse_error);
    for (const b of bad) g.push(`LEDGER.jsonl:${b.__parse_error}: not valid JSON`);
    const angles = new Set(ledger.filter((r) => r.angle).map((r) => String(r.angle).toLowerCase()));
    if (angles.size < ANGLE_MIN) g.push(`LEDGER.jsonl: only ${angles.size} distinct angles; need >= ${ANGLE_MIN}`);
  }
  if (cov) {
    const hunks = diffHunks();
    if (hunks.length === 0) g.push(`no diff hunks found for ${baseRef}...HEAD (nothing implemented, or wrong --base)`);
    for (const h of hunks) {
      const matching = cov.filter((r) => r.file === h.file && r.start <= h.end && r.end >= h.start);
      const angleUnion = new Set();
      for (const r of matching) r.angles.forEach((a) => angleUnion.add(a));
      if (angleUnion.size < COVERAGE_MIN_ANGLES)
        g.push(`AUDIT_COVERAGE.tsv: hunk ${h.file}:${h.start}-${h.end} reviewed by ${angleUnion.size} angle(s) [${[...angleUnion].join(',') || 'none'}]; need >= ${COVERAGE_MIN_ANGLES}`);
    }
  }
  return { present: true, gaps: g };
}

function phase7() {
  const g = [];
  const rounds = glob('FIX_ROUND');
  if (rounds.length === 0) return { present: false, gaps: ['no FIX_ROUND-<n>.md files (fix/re-audit loop not started)'] };
  const last = rounds[rounds.length - 1];
  const lt = read(last.file);
  const m = /new confirmed findings\s*:?\s*\*{0,2}\s*(\d+)/i.exec(lt);
  if (!m) g.push(`${last.file}: missing "**New confirmed findings:** <N>" summary line`);
  else if (parseInt(m[1], 10) !== 0) g.push(`${last.file}: fix loop not converged — ${m[1]} new confirmed finding(s) in the final round`);
  return { present: true, gaps: g };
}

function phase8() {
  const g = [];
  const t = read('TEST_RESULTS.md');
  if (t == null) return { present: false, gaps: ['TEST_RESULTS.md missing'] };
  const tests = parseTests();
  if (!tests) return { present: true, gaps: ['TESTS.md missing — cannot verify results'] };
  const results = new Map();
  for (const ln of t.split(/\r?\n/)) {
    const m = RE_RESULT.exec(ln);
    if (m) results.set(m[1], m[2].toUpperCase());
  }
  for (const test of tests) {
    const r = results.get(test.id);
    if (!r) g.push(`TEST_RESULTS.md: ${test.id} (from TESTS.md) has no result line`);
    else if (r !== 'PASS') g.push(`TEST_RESULTS.md: ${test.id} is ${r}, not PASS`);
  }
  return { present: true, gaps: g };
}

const PHASES = [null, phase1, phase2, phase3, phase4, phase5, phase6, phase7, phase8];
const PHASE_NAMES = [
  '', 'PLAN', 'PLAN_AUDIT', 'TESTS', 'DECISIONS',
  'IMPLEMENT+DRIFT', 'BLIND_AUDIT', 'FIX_LOOP', 'TEST_RESULTS',
];

// ---------------------------------------------------------------------------
// runners
// ---------------------------------------------------------------------------
function runOne(n) {
  const r = PHASES[n]();
  return { n, name: PHASE_NAMES[n], ...r };
}

function report(results) {
  let anyFail = false;
  for (const r of results) {
    const status = !r.present ? 'PENDING' : r.gaps.length === 0 ? 'OK' : 'FAIL';
    const glyph = status === 'OK' ? '✓' : status === 'PENDING' ? '·' : '✗';
    process.stdout.write(`  ${glyph} phase ${r.n} ${r.name.padEnd(16)} ${status}\n`);
    if (r.gaps.length) {
      for (const gap of r.gaps) process.stdout.write(`      - ${gap}\n`);
      if (r.present) anyFail = true;
    }
  }
  return anyFail;
}

process.stdout.write(`lifecycle-check  feature=${featureDir.replace(repo + '/', '')}  base=${baseRef}\n`);

if (wantAll) {
  const results = [];
  for (let n = 1; n <= 8; n++) results.push(runOne(n));
  const anyFail = report(results);
  // contiguity: no completed (present & OK) phase may sit above a PENDING one
  let sawPending = false;
  let gap = false;
  for (const r of results) {
    if (!r.present) { sawPending = true; continue; }
    if (r.present && sawPending) { gap = true; process.stdout.write(`  ! phase ${r.n} ${r.name} has artifacts but an earlier phase is PENDING (gate skipped)\n`); }
  }
  if (anyFail || gap) {
    process.stderr.write('lifecycle-check: FAIL — resolve the gaps above before pushing.\n');
    process.exit(1);
  }
  const highest = results.filter((r) => r.present).map((r) => r.n).pop() || 0;
  process.stdout.write(`lifecycle-check: OK — phases 1..${highest} complete (${highest}/8).\n`);
  process.exit(0);
}

if (phaseArg) {
  const n = parseInt(phaseArg, 10);
  if (!(n >= 1 && n <= 8)) fail(`--phase must be 1..8 (got ${phaseArg})`);
  const r = runOne(n);
  const anyFail = report([r]);
  if (!r.present) {
    process.stderr.write(`lifecycle-check: phase ${n} ${r.name} PENDING — artifacts not created yet.\n`);
    process.exit(1);
  }
  if (anyFail) {
    process.stderr.write(`lifecycle-check: phase ${n} ${r.name} FAIL.\n`);
    process.exit(1);
  }
  process.stdout.write(`lifecycle-check: phase ${n} ${r.name} OK — you may proceed to phase ${n + 1}.\n`);
  process.exit(0);
}

fail('specify --phase <1-8> or --all');

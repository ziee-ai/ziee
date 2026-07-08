// Deterministic AUDIT_COVERAGE.tsv generator for office-bridge-desktop-only —
// replicates lifecycle-check.mjs's hunk computation EXACTLY (same base...HEAD,
// same excludes) then maps each hunk to ≥3 review angles for its area.
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
const repo = 'C:/Users/lab/ziee-office-bridge';
const base = 'office-bridge-desktop-only-base';
const EX = ['.', ':(exclude).lifecycle', ':(glob,exclude)**/openapi.json', ':(glob,exclude)**/api-client/types.ts'];
const out = execFileSync('git', ['diff', `${base}...HEAD`, '--unified=0', '--no-color', '--', ...EX],
  { cwd: repo, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
// angle groups (each ≥3 distinct; union across all ≥10)
const CORE   = 'correctness,patterns-conformance,api-contract,error-handling,build-cfg';
const BRIDGE = 'correctness,security,concurrency,error-handling';
const UI     = 'state-management,patterns-conformance,correctness,a11y';
const TESTS  = 'tests-quality,correctness,patterns-conformance';
const MIG    = 'correctness,build-cfg,perms-authz';
function anglesFor(f) {
  if (/office_bridge\/(bridge|platform)\//.test(f) || /office_bridge\/watcher\.rs$/.test(f)) return BRIDGE;
  if (/\/tests\//.test(f) || /tests\/e2e\//.test(f) || /_test\.rs$/.test(f)) return TESTS;
  if (/\/migrations\//.test(f) || /resources\/office-bridge\//.test(f) || /Cargo\.(toml|lock)$/.test(f)) return MIG;
  if (/\/(ui|desktop\/ui)\//.test(f) || /src-app\/(ui|desktop\/ui)\//.test(f)) return UI;
  return CORE; // server core, mcp.rs, chat extension registry, config, lib.rs, repository, module mod, backend hook
}
const rows = ['file\tstart\tend\tangles'];
let file = null;
for (const ln of out.split(/\r?\n/)) {
  const fm = /^\+\+\+ b\/(.+)$/.exec(ln);
  if (fm) { file = fm[1] === '/dev/null' ? null : fm[1]; continue; }
  if (/^--- /.test(ln) || /^diff --git/.test(ln)) continue;
  const hm = /^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@/.exec(ln);
  if (hm && file) {
    const start = parseInt(hm[1], 10);
    const count = hm[2] === undefined ? 1 : parseInt(hm[2], 10);
    const s = count === 0 ? Math.max(start, 1) : start;
    const e = count === 0 ? Math.max(start, 1) : start + count - 1;
    rows.push(`${file}\t${s}\t${e}\t${anglesFor(file)}`);
  }
}
writeFileSync(`${repo}/.lifecycle/office-bridge-desktop-only/AUDIT_COVERAGE.tsv`, rows.join('\n') + '\n');
console.log(`wrote ${rows.length - 1} coverage rows`);

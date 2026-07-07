// Deterministic AUDIT_COVERAGE.tsv generator — replicates lifecycle-check.mjs's
// hunk computation exactly, then maps each hunk to its area's review angles.
import { execFileSync } from 'node:child_process';
import { writeFileSync } from 'node:fs';
const repo = 'C:/Users/lab/ziee-office-bridge';
const base = 'origin/main';
const EX = ['.', ':(exclude).lifecycle', ':(glob,exclude)**/openapi.json', ':(glob,exclude)**/api-client/types.ts'];
const out = execFileSync('git', ['diff', `${base}...HEAD`, '--unified=0', '--no-color', '--', ...EX],
  { cwd: repo, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 });
// area -> angles
const A = 'correctness,security,perms-authz,api-contract,error-handling';
const B = 'correctness,security,concurrency,error-handling,perf';
const C = 'state-management,a11y,patterns-conformance,i18n-copy,correctness';
const D = 'tests-quality,correctness,security';
function anglesFor(f) {
  if (/office_bridge\/(bridge|platform)\//.test(f) || /office_bridge\/watcher\.rs$/.test(f)) return B;
  if (/^src-app\/ui\//.test(f)) return C;
  if (/office-bridge\/(manifest|taskpane|icon)/.test(f) || /resources\/office-bridge\//.test(f)
      || /server\/tests\//.test(f) || /tests\/e2e\//.test(f)) return D;
  return A; // backend core, mcp.rs, sync, migrations, config, Cargo, lib.rs
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
writeFileSync(`${repo}/.lifecycle/office-bridge/AUDIT_COVERAGE.tsv`, rows.join('\n') + '\n');
console.log(`wrote ${rows.length - 1} coverage rows`);

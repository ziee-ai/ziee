// Node unit test for taskpane.js's PURE string/path helpers — the logic the blind
// audit flagged as otherwise only manually verifiable. The Office.js op handlers
// (Word.run/Excel.run/…) still require a real host and are covered by the live
// MAC/WINDOWS checklists; this covers baseName / isPathLike / normPath / sameDoc /
// capText (the mis-routing guard + read cap), which are host-independent.
//
//   node src-app/desktop/tauri/resources/office-bridge/taskpane.test.mjs

import { createRequire } from 'node:module';
import assert from 'node:assert/strict';

const require = createRequire(import.meta.url);
const t = require('./taskpane.js');

// baseName — last path segment, / and \ tolerant.
assert.equal(t.baseName('/Users/x/Report.docx'), 'Report.docx');
assert.equal(t.baseName('C:\\Users\\x\\Report.docx'), 'Report.docx');
assert.equal(t.baseName('Book1'), 'Book1');
assert.equal(t.baseName(''), '');

// isPathLike — has a separator.
assert.equal(t.isPathLike('/Users/x/R.docx'), true);
assert.equal(t.isPathLike('C:\\x\\R.docx'), true);
assert.equal(t.isPathLike('Document1'), false);

// normPath — strip file://, unify separators, lowercase, and de-slash a Windows
// drive letter so a file URL matches the native COM path.
assert.equal(t.normPath('/Users/x/Report.docx'), '/users/x/report.docx');
assert.equal(t.normPath('file:///Users/x/Report.docx'), '/users/x/report.docx');
assert.equal(t.normPath('C:\\Users\\x\\Report.docx'), 'c:/users/x/report.docx');
assert.equal(t.normPath('file:///C:/Users/x/Report.docx'), 'c:/users/x/report.docx');

// sameDoc — full-path compare when both path-like (cross-dir collision rejected),
// basename fallback otherwise; format + case differences on the SAME doc match.
assert.equal(t.sameDoc('/work/Report.docx', '/personal/Report.docx'), false, 'cross-dir must NOT match');
assert.equal(t.sameDoc('/Users/x/Report.docx', 'file:///Users/x/Report.docx'), true, 'native vs file URL, same doc');
assert.equal(t.sameDoc('C:\\Users\\x\\Report.docx', 'file:///C:/Users/x/Report.docx'), true, 'windows native vs file URL');
assert.equal(t.sameDoc('/Users/x/Report.docx', '/Users/x/REPORT.DOCX'), true, 'case-insensitive same doc');
assert.equal(t.sameDoc('Book1', 'Book1'), true, 'bare unsaved names match');

// capText — under the cap is untouched; over the cap is sliced AND carries an
// in-band truncation marker in the text channel.
const small = t.capText('hello');
assert.equal(small.truncated, false);
assert.equal(small.text, 'hello');
const big = t.capText('a'.repeat(t.MAX_READ_CHARS + 50));
assert.equal(big.truncated, true);
assert.ok(big.text.length <= t.MAX_READ_CHARS + 100);
assert.ok(big.text.includes('[truncated'), 'truncated text carries an in-band marker');

// serializeResult (TEST-9) — the run_office_js return-value shaping (DEC-7).
// A small JSON value round-trips to its native shape, truncated=false.
const num = t.serializeResult(42);
assert.equal(num.result, 42, 'number round-trips as a native number');
assert.equal(num.truncated, false);
const obj = t.serializeResult({ address: 'A1', ok: true });
assert.deepEqual(obj.result, { address: 'A1', ok: true }, 'object round-trips as a native object');
assert.equal(obj.truncated, false);
const str = t.serializeResult('hello');
assert.equal(str.result, 'hello', 'string round-trips');
// `undefined` (no return) → null, not a throw.
const undef = t.serializeResult(undefined);
assert.equal(undef.result, null, 'undefined → null');
assert.equal(undef.truncated, false);
// An over-cap return is truncated (result stays the capped string) and never throws.
const huge = t.serializeResult('x'.repeat(t.MAX_READ_CHARS + 50));
assert.equal(huge.truncated, true, 'over-cap return is flagged truncated');
assert.equal(typeof huge.result, 'string', 'truncated result is the capped string');
assert.ok(huge.text.includes('[truncated'), 'truncated payload carries the in-band marker');
// A circular / non-serializable value degrades to a string WITHOUT throwing.
const circular = {};
circular.self = circular;
const circ = t.serializeResult(circular);
assert.equal(typeof circ.result, 'string', 'circular value degrades to a readable string');
assert.equal(circ.truncated, false);

// describeError (DEC-9) — structured Office.js error string incl. code + debugInfo.
const de = t.describeError('run_office_js', {
  name: 'RichApi.Error',
  message: 'The range does not exist.',
  code: 'ItemNotFound',
  debugInfo: { errorLocation: 'Range.getRange' },
});
assert.ok(de.includes('run_office_js failed'), 'prefixed');
assert.ok(de.includes('The range does not exist.'), 'carries the message');
assert.ok(de.includes('ItemNotFound'), 'carries the Office.js error code');
assert.ok(de.includes('debugInfo'), 'carries debugInfo');

console.log('taskpane.test.mjs: all pure-helper assertions passed');

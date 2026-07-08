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

console.log('taskpane.test.mjs: all pure-helper assertions passed');

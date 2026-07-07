import { test } from 'node:test'
import assert from 'node:assert/strict'
import { EMAIL_RE, isValidEmail } from './validation.ts'

// ── EMAIL_RE: the regression the shared validator fixes ──────────────────────
// The Vite/Oxc minifier corrupts open-ended `{n,}`→`{n}` in regex literals, so
// Zod's `.email()` shipped as `[A-Za-z]{2}$` and rejected every 3+char TLD.
// These cases lock in acceptance of real TLDs and rejection of malformed input.

test('EMAIL_RE accepts valid addresses incl. 3+char TLDs and subdomains', () => {
  for (const email of [
    'khoi@gmail.com', // the reported 3-char-TLD regression
    'khoi@tinnguyen-lab.com', // hyphen in domain label
    'abc@gmail.co', // 2-char TLD still valid
    'user.name@sub.example.org',
    'a@b.io',
    'x+tag@example.info',
    'first.last@many.sub.domains.example.net',
  ]) {
    assert.equal(isValidEmail(email), true, `expected valid: ${email}`)
  }
})

test('EMAIL_RE rejects malformed addresses', () => {
  for (const email of [
    'a@.com', // leading dot in domain
    'a@b..com', // consecutive dots
    'abc@gmail', // no TLD / no dot
    'a@@b.com', // double @
    'a b@c.com', // whitespace
    'plainaddress', // no @
    '@no-local.com', // empty local part
    'trailing@dot.', // trailing dot
    'a@-bad.com', // label starts with hyphen
    'a@bad-.com', // label ends with hyphen
    'a@b.c1', // non-alphabetic TLD
  ]) {
    assert.equal(isValidEmail(email), false, `expected invalid: ${email}`)
  }
})

test('EMAIL_RE is a bounded regex literal (no open-ended quantifier)', () => {
  // Guards against a future edit reintroducing `{n,}`, which the prod minifier
  // corrupts. Source must contain no open-ended quantifier.
  assert.equal(/\{\d+,\}/.test(EMAIL_RE.source), false)
})

import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  FALLBACK_APPROVAL_MODE,
  approvalModePayload,
  blankMcpConfig,
  effectiveApprovalMode,
  type ApprovalModeValue,
} from './approvalDefaults.ts'

const ALL_MODES: ApprovalModeValue[] = ['disabled', 'auto_approve', 'manual_approve']

// ── TEST-8: blankMcpConfig carries the SERVER default ────────────────────────

test('blankMcpConfig stamps the supplied server default onto approvalMode', () => {
  // Checked for ALL three modes, so a hardcoded 'manual_approve' could not pass.
  for (const mode of ALL_MODES) {
    assert.equal(blankMcpConfig(mode).approvalMode, mode)
  }
})

test('blankMcpConfig starts empty and falls back safely with no server default', () => {
  const config = blankMcpConfig()
  assert.equal(config.approvalMode, FALLBACK_APPROVAL_MODE)
  assert.equal(config.selectedServers.size, 0)
  assert.deepEqual(config.disabledServers, [])
  assert.deepEqual(config.autoApprovedTools, [])
  // loopSettings is omitted (not null) so the backend's own default applies.
  assert.equal('loopSettings' in config, false)
})

test('blankMcpConfig returns fresh collections per call (no aliasing between configs)', () => {
  const a = blankMcpConfig('auto_approve')
  const b = blankMcpConfig('auto_approve')
  a.selectedServers.set('s1', { server_id: 's1', tools: [] })
  a.disabledServers.push({ server_id: 's2', tools: [] })
  a.autoApprovedTools.push({ server_id: 's3', tools: ['t'] })
  assert.equal(b.selectedServers.size, 0)
  assert.deepEqual(b.disabledServers, [])
  assert.deepEqual(b.autoApprovedTools, [])
})

test('blankMcpConfig passes through loop settings when supplied', () => {
  const config = blankMcpConfig('manual_approve', { max_iteration: 3 })
  assert.deepEqual(config.loopSettings, { max_iteration: 3 })
})

// ── TEST-9: effectiveApprovalMode — explicit choice always wins ──────────────

test('effectiveApprovalMode returns the config mode whenever one is set', () => {
  for (const mode of ALL_MODES) {
    for (const serverDefault of ALL_MODES) {
      assert.equal(
        effectiveApprovalMode(mode, serverDefault),
        mode,
        `explicit ${mode} must survive a ${serverDefault} server default`,
      )
    }
  }
})

test('effectiveApprovalMode falls back to the server default when unset', () => {
  for (const serverDefault of ALL_MODES) {
    assert.equal(effectiveApprovalMode(undefined, serverDefault), serverDefault)
    assert.equal(effectiveApprovalMode(null, serverDefault), serverDefault)
  }
})

test('effectiveApprovalMode uses the safe fallback only when the server default is unknown', () => {
  assert.equal(effectiveApprovalMode(undefined), FALLBACK_APPROVAL_MODE)
  // The safe fallback is the RESTRICTIVE mode — never auto-approve on a guess.
  assert.equal(FALLBACK_APPROVAL_MODE, 'manual_approve')
})

// ── TEST-10: approvalModePayload — omit, never guess ─────────────────────────

test('approvalModePayload omits the key entirely when no mode is set', () => {
  for (const unset of [undefined, null] as const) {
    const payload = approvalModePayload(unset)
    assert.equal(
      'approval_mode' in payload,
      false,
      'an un-customized save must not send approval_mode at all — the server ' +
        'COALESCEs to its own default on insert and preserves on update',
    )
    assert.deepEqual(payload, {})
  }
})

test('approvalModePayload includes the mode when one is explicitly set', () => {
  for (const mode of ALL_MODES) {
    assert.deepEqual(approvalModePayload(mode), { approval_mode: mode })
  }
})

test('approvalModePayload spreads into a request body without leaking a key', () => {
  const unset = { ...approvalModePayload(undefined), disabled_servers: [] }
  assert.deepEqual(Object.keys(unset).sort(), ['disabled_servers'])

  const set = { ...approvalModePayload('auto_approve'), disabled_servers: [] }
  assert.deepEqual(Object.keys(set).sort(), ['approval_mode', 'disabled_servers'])
})

import type { AutoApprovedServer, DisabledServer, LoopSettings } from '@/api-client/types'

/**
 * Where a blank MCP config's approval mode comes from — the pure logic behind the
 * "auto-approve doesn't survive past turn 1" fix, extracted so it is unit-testable
 * WITHOUT importing the enum-laden `McpComposer.store` (node's strip-only type mode
 * rejects enums), mirroring `approvalRouting.ts`.
 *
 * ## Why this exists
 *
 * The approval mode a brand-new conversation gets is the SERVER's decision, not the
 * client's. Previously the store hardcoded `'manual_approve'` in ~10 places: it
 * displayed that in the config modal and then PERSISTED it on the first send, which
 * silently downgraded a deployment configured to auto-approve — the conversation
 * auto-approved on turn 1 (no stored row, server default applied) and prompted from
 * turn 2 on (client-written row saying manual).
 *
 * So: the server reports its default as `default_approval_mode` on
 * `GET /api/mcp/defaults`; the store keeps it in `serverDefaultApprovalMode`; and
 * every blank config + every display fallback reads it from here.
 *
 * On WRITES the rule is different and stricter — see {@link approvalModePayload}: an
 * un-customized save OMITS the field rather than echoing the default back, so the
 * server stays authoritative.
 */

/** The three approval modes, as the API spells them. */
export type ApprovalModeValue = 'disabled' | 'auto_approve' | 'manual_approve'

/**
 * Last-resort mode for when the server default is genuinely unknown — before
 * `GET /api/mcp/defaults` resolves, or after it fails (`loadUserDefaults` swallows
 * its error and still marks itself loaded).
 *
 * Fail SAFE: if we can't know, assume the more restrictive mode. Worst case the user
 * is asked to approve a tool that would have auto-run, which is recoverable; the
 * inverse silently runs a third-party tool without consent.
 *
 * This value is never PUT — an unset mode is omitted from the request body instead.
 */
export const FALLBACK_APPROVAL_MODE: ApprovalModeValue = 'manual_approve'

/** The shape of a per-conversation / per-project MCP config's blank state. */
export interface BlankMcpConfig {
  selectedServers: Map<string, { server_id: string; tools: string[] }>
  disabledServers: DisabledServer[]
  approvalMode: ApprovalModeValue
  autoApprovedTools: AutoApprovedServer[]
  loopSettings?: LoopSettings
}

/**
 * A fresh, un-customized MCP config carrying the SERVER's default approval mode.
 *
 * Replaces the `approvalMode: 'manual_approve'` literal that was duplicated across
 * every config-creation site in the store. Returns fresh collections each call so
 * two configs can never alias the same `Map`/array.
 */
export function blankMcpConfig(
  serverDefault: ApprovalModeValue = FALLBACK_APPROVAL_MODE,
  loopSettings?: LoopSettings,
): BlankMcpConfig {
  return {
    selectedServers: new Map(),
    disabledServers: [],
    approvalMode: serverDefault,
    autoApprovedTools: [],
    ...(loopSettings !== undefined ? { loopSettings } : {}),
  }
}

/**
 * The approval mode to DISPLAY for a config: its own value when it has one,
 * otherwise the server default.
 *
 * An explicit user choice always wins, including when it differs from the server
 * default — picking Manual on an auto-approving deployment must stick.
 */
export function effectiveApprovalMode(
  configMode: ApprovalModeValue | undefined | null,
  serverDefault: ApprovalModeValue = FALLBACK_APPROVAL_MODE,
): ApprovalModeValue {
  return configMode ?? serverDefault
}

/**
 * The `approval_mode` fragment of a settings PUT body.
 *
 * Returns `{}` — i.e. OMITS the key entirely — when the config has no explicit mode,
 * so the server applies its own default on insert and preserves the stored value on
 * update (both via `COALESCE`, see `approval/repository.rs`). Spread into the request
 * body: `{ ...approvalModePayload(config.approvalMode), disabled_servers }`.
 *
 * Omitting rather than echoing the server default back matters: it keeps the SERVER
 * authoritative, so a stale cached bundle, a failed defaults fetch, or a third-party
 * API client can't downgrade a conversation with a value it guessed. Mirrors how the
 * same PUT already treats `auto_approved_tools`.
 */
export function approvalModePayload(
  configMode: ApprovalModeValue | undefined | null,
): { approval_mode?: ApprovalModeValue } {
  return configMode ? { approval_mode: configMode } : {}
}

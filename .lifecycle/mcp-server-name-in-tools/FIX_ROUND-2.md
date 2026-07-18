# FIX_ROUND-2

## Fix applied

- **Elicitation call-site consistency (LOW, from FIX_ROUND-1 re-audit).** The
  built-in `ask_user` branch (`mcp.rs`) passed `(!server.is_built_in).then(|| server.name.as_str())`
  — a raw, unsanitized `server.name` as the label candidate. It is dead today
  (the branch is guarded by `*server_id == elicitation_mcp_server_id()`, whose row
  is always `is_built_in = true`, so it yields `None`), but it was a latent footgun.
  **Fix:** pass `None` directly, since this branch is definitionally the built-in
  elicitation server and its tools are never labeled. No behavior change.

## Findings recorded as accepted residuals (not fixed — see LEDGER)

- **Cf format chars survive the sanitizer** (bidi overrides, zero-width) — LOW,
  non-structural (cannot forge a heading/newline; those are blocked). Stripping all
  Cf would mangle legitimate ZWJ/ZWNJ names (Persian/Indic/emoji), so left as-is.
- **Inline prose in a description is not escaped** — MEDIUM per the adversarial
  auditor, but inherent: the feature's purpose is to SHOW the admin/user-set
  description so the model can answer "what is <server>". These fields are set by
  the admin/user (NOT the remote MCP server), who already control the system prompt
  directly, and the value is now confined to a single line. This matches the
  codebase's untrusted-content posture (a guard nudge, not field escaping). Flagged
  to the human for phase-9 review rather than silently mitigated with prompt bloat.

## Re-audit (fresh blind round on the post-round-2 diff)

A fresh blind auditor reviewed the complete diff after the elicitation fix.

**New confirmed findings:** 0

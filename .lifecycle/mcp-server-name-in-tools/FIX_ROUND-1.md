# FIX_ROUND-1

## Findings fixed (from the phase-6 blind audit)

- **SEC-1 (security/i18n, confirmed)** — `server.name` / `server.description` reached
  the model-visible prompt unsanitized for the first time; a newline could forge a
  `## ` heading in the system message (worst case: an admin-set *system* server
  injecting into every group member's prompt). **Fix:** added
  `sanitize_prompt_field(&str, cap)` — maps control chars to space, collapses all
  Unicode whitespace runs (incl. U+2028/U+2029/NEL, which `char::is_whitespace()`
  covers) to single spaces via `split_whitespace().join(" ")`, and caps length by
  CHAR count with an ellipsis. Applied to the `[name]` label (once per external
  server) and to the roster name + description. New unit test TEST-6
  (`sanitize_prompt_field_collapses_and_caps`) proves a `biognosia\n\n## System …`
  name collapses to one inert line and the cap holds. Tracked as ITEM-5.

- **F1 (perf, confirmed)** — the labeled path cloned `mcp_tool.description` then
  discarded it. **Fix:** clone only in the `None` arm; the labeled arm formats from
  `as_deref()`.

- **F2 (efficiency, confirmed)** — the roster tuple (two String clones per external
  server) was built on every tool-loop iteration but consumed only at iteration 1.
  **Fix:** gate the `external_servers.push` on `context.iteration == 1`.

## Findings rejected (with rationale — recorded in LEDGER.jsonl)

- **`(1 tools)` pluralization** — cosmetic prompt text the model does not act on; left
  as-is, noted in STATUS.
- **`(0 tools)` roster line** for an all-guard-dropped external server — intentional
  ("a connected server is worth naming for 'what is <server>'"); not a leak.
- **TEST-5 passes with/without the label** — by design (wire name intentionally
  unchanged); it is a non-regression guard for dispatch, not a label assertion.
- **No drop-count test** — the `advertised` counter is trivial (increments only on a
  successful convert); accepted low-value gap, noted in STATUS.

## Re-audit (fresh blind round on the full post-fix diff)

Two fresh blind auditors reviewed the complete post-fix diff (a full-angle
convergence pass + an adversarial pass dedicated to bypassing
`sanitize_prompt_field`).

- **Convergence auditor:** diff CLEAN — no defect across correctness, security,
  prompt-injection, error-handling, authz, api-contract, patterns, perf,
  test-reality. Confirmed U+2028/U+2029 are caught by `split_whitespace()`.
- **Adversarial auditor:** all STRUCTURAL injection classes (heading forge, line
  breakout incl. U+2028/U+2029, length/multibyte) genuinely BLOCKED. Residuals:
  - **elicitation call site passed raw `server.name`** (LOW, latent/dead — always
    `None` because elicitation is built-in). → **CONFIRMED, fixed in FIX_ROUND-2**
    (pass `None` directly).
  - **Cf format chars survive** (bidi/zero-width; LOW, non-structural). → REJECTED
    as accepted residual (stripping Cf risks mangling legitimate ZWJ/ZWNJ names).
  - **inline prose not escaped** (MEDIUM per the auditor). → REJECTED as inherent:
    the feature must SHOW the description; fields are admin/user-set (not
    remote-server-supplied); matches the codebase untrusted-content posture.
    Flagged to the human for phase-9 review.

**New confirmed findings:** 1 (elicitation raw-name — fixed in round 2)

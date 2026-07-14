# Chunk D — TRANSFORMS (design decisions on the created SDK types)

No existing app symbol was moved/edited this chunk (see CUT.md), so there are no
byte-identical-modulo-transform moves to declare. The transforms below are the
**design decisions** baked into the net-new harness types — the DECISIONS-analog
for the two design-gates.

## T1 — Capability manifest: STRUCTURE in harness, CONTENTS app-side

The four mode-gating mechanisms are unified by one `CapabilityManifest` keyed by
`DeploymentMode`. The manifest **fields** map 1:1 to the four mechanisms:

| Mechanism (today) | Field |
|---|---|
| backend `create_desktop_modules` vec | `backend_modules: Vec<String>` |
| frontend `CORE_MODULE_BLOCKLIST` Set | `frontend_blocklist: Vec<String>` |
| scattered `config.<f>.enabled = true` | `config_overrides: BTreeMap<String,bool>` |
| `setMultiUserMode(false)` | `multi_user: bool` |

Decision: only the **shape + mode keying** are reusable (harness). The **contents**
(which modules, which blocklist ids, which overrides) stay app-side, matching
Chunk D's "stays app-side" list. Frontend parts (2 + 4) are exposed via
`FrontendManifest`, which the app serves so the desktop loader stops being a
second hand-forked source of truth.

## T2 — Single-user strategy: concrete mint/read half, seam for creation

`SingleUserStrategy::{mint_owner_login, owner_missing, owner_permissions}` are
**concrete** and reproduce the app's `mint_admin_login` / `ensure_desktop_admin`
semantics exactly:
- mint via `ziee_auth::auth::refresh_tokens::mint_session_tokens` (the shared
  jti-whitelisted path — desktop sessions revocable + prunable like server ones);
- owner lookup via `ziee_auth::user::UserRepository`;
- the "Admin not found - server may still be starting" error string preserved.

Decision: owner **creation** (`create_admin_user` + the Administrators-`*` grant)
stays app-side (BA kept domain admin CRUD app-side), so `owner_missing` is the
readable half here and creation routes through the `ServerBoot` seam. The owner
identity is parameterized (`desktop_default()` reproduces `"admin"` verbatim) so
a second app picks its own owner (pluggable identity, decision #1).

## T3 — owner-`*` = the existing `is_admin` short-circuit + `"*"` wildcard

`OWNER_WILDCARD_PERMISSION = "*"` is the single named constant tying the app's
Administrators-group grant to the harness. Desktop's owner is `is_admin`, and
`ziee-identity`'s RBAC evaluator (the `is_admin` short-circuit +
`check_permissions_array` `"*"` match, `crates/ziee-identity/src/rbac.rs`)
satisfies every `RequirePermissions<...>` gate. No new enforcement code — the
strategy just **selects** the identity crate's existing behavior, so
permission-gated code stays written-once for both modes.

## T4 — `ServerBoot` seam instead of moving `start_server_with_routes`

`start_server_with_routes` is the app's entire (non-agnostic) server assembly, so
it stays app-side; the harness receives a booted server via
`ServerBoot::boot() -> BootHandle{ addr, pool, jwt }`. This is the BG-3 target
(see STOP_REPORT.md) — an injected seam, the same posture BG used for auth.

## Equivalence angle

Golden (openapi.json + types.ts, both surfaces) is unaffected **by construction**:
zero app-side edits, and nothing in the app graph depends on the harness yet
(no path dep wired), so the app's generated artifacts cannot change.

## Security angle (auth / single-user)

- The harness mints via the **shared** `mint_session_tokens` (jti whitelist),
  not a bespoke token path — desktop sessions inherit revocation
  (logout-everywhere) + pruning + the admin-configurable lifetimes.
- owner-`*` is the **existing** `is_admin`/`"*"` evaluator, not a new bypass; no
  new privilege path is introduced.
- No secret material or JWT secret is embedded in the harness; the JWT service
  arrives at runtime through `BootHandle` (the app's per-boot JWT-secret policy
  stays app-side). The harness has no `query!` macros ⇒ no build DB, no schema
  coupling, no secret in the SDK build lane.

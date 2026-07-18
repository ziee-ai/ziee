# Chunk D — BOUNDARY (exit checklist)

Chunk D is **PARTIAL**: design-gates delivered as green SDK types; the live
Tauri-shell MOVE + embedded-PG relocation are STOPPED on the BG-3 prerequisite
(`STOP_REPORT.md`). No app-side code was cut, so the app tree is untouched.

## Gate results

| Gate | Result |
|---|---|
| `cargo check -p ziee-desktop-harness` | **exit 0** |
| `cd sdk && cargo check --workspace` | **exit 0** (harness added as a member; ziee-auth auth-only build DB warning only) |
| `cargo test -p ziee-desktop-harness --lib` | **6 passed / 0 failed** (manifest ×4, single_user ×2) |
| `cargo check -p ziee` (lib + `ziee` bin) | **exit 0** — only the pre-existing dead-code warnings BA left; zero new |
| `cargo check -p ziee-desktop` | **exit 0** — pre-existing warnings only |
| Desktop boot intact (2 IPC commands + `auto_login` path) | **UNCHANGED** — `ziee-desktop` not edited; `get_server_port` + `auto_login` + the auto-login path are byte-for-byte as BA left them |
| E8 golden — `types.ui.ts` | **BYTE-IDENTICAL by construction** — zero app-side diff; nothing in the app graph depends on the harness (no path dep wired) |
| E8 golden — `types.desktop.ts` | **BYTE-IDENTICAL by construction** (same reason) |
| E8 golden — `openapi.ui.json` | **CANONICALLY-EQUAL by construction** |
| E8 golden — `openapi.desktop.json` | **CANONICALLY-EQUAL by construction** |

**Golden rationale.** The entire diff is inside the SDK submodule's
`desktop/harness` crate. `ziee-desktop/Cargo.toml` does not (yet) depend on
`ziee-desktop-harness` and `ziee` never will, so the two apps' generated
`openapi.json` + `types.ts` are provably unreachable from this change. A full
regen would rebuild the ziee + ziee-desktop bins only to reproduce BA's last
green result (all four surfaces BYTE-IDENTICAL / CANONICALLY-EQUAL); it was not
re-run because the input is unchanged.

## Design-gates — status

- **Design-gate 1 (4-part capability manifest keyed by mode):** DELIVERED as
  `manifest::CapabilityManifest` (+ `DeploymentMode`, `FrontendManifest`). Wiring
  it into ziee-desktop (replacing the four live mechanisms) is part of BG-3 —
  invasive + frontend-touching, deferred with the shell MOVE.
- **Design-gate 2 (single-user strategy + owner-`*`):** DELIVERED as
  `single_user::SingleUserStrategy` on `ziee-auth`'s mint path + `ziee-identity`'s
  `"*"` RBAC. Concrete mint/read; creation routes through the seam.

## Globals — how each was handled

Per the fail-safe, the desktop-consumer globals (`ziee::Repos`, the JWT
`OnceLock`, config statics, the `"admin"`/`is_admin` assumption) are **not**
force-injected here. They are captured behind the specified `ServerBoot` seam
(`boot.rs`) and reported as the **BG-3 prerequisite** — the same shape as BG→BA.

## Committed (SDK submodule only)

The SDK side (`ziee-desktop-harness` Cargo.toml/lib.rs + `manifest.rs` /
`single_user.rs` / `boot.rs` + `Cargo.lock`) is committed in `sdk/` — NOT pushed.
The `.extraction/D/` artifacts + the `ORDER` append are on the ziee-branch side,
left uncommitted for the orchestrator (matching the BG/BA convention). No remote
pushed; `/data/pbya/ziee/ziee` untouched.

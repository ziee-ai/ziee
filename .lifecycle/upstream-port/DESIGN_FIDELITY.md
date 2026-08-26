# DESIGN_FIDELITY — upstream-port

The invariants are lifted from the paws commits that authored each fix. For a port
UPWARD the fidelity question is sharper than usual: paws' version of each fix is
entangled with paws' product direction, so "upheld" must mean **the defect is closed
in upstream's shape**, not "the paws diff was copied".

- **INV-1** — fidelity: UPHELD — verified by running, not by reading. `cargo check -p
  ziee-desktop` on untouched `upstream/main` fails `error[E0063]: missing field
  enable_popout_windows` at `mod.rs:302`; with ITEM-1 the whole workspace checks
  clean (`cargo check --workspace --all-targets`, exit 0). The release-only half (the
  un-cfg-gated axum import under `unused_imports = "deny"`) is fixed by the same
  item.
- **INV-2** — fidelity: UPHELD — `register_routes` now guards in all three modules.
  Upstream's own `VoiceModule` already did; this makes the set consistent rather than
  introducing a new pattern.
- **INV-3** — fidelity: UPHELD — all four module structs (`js_tool`, `web_search`,
  `lit_search`, and `voice`, whose `new()` still defaulted `true`) now initialise
  `enabled: false`, so `register_routes` fails CLOSED on any path where `init()` never
  ran. Note this is the ONLY part of paws' voice change taken: paws' default FLIP
  (`voice_enabled()`) is product direction and is not ported.
- **INV-4** — fidelity: UPHELD, with the rationale re-derived for upstream. paws
  justifies keeping the settings REST mounted because its admin UI module is not
  hidden. Upstream hides nothing, so that reason does not transfer; the reason that
  DOES is the one now in the code — those routes only read and write configuration,
  nothing there egresses a query, and an admin page should keep working (and keep
  showing the feature as off) on a deployment that disabled it. Same split, honest
  reason.
- **INV-5** — fidelity: UPHELD, and this is the one invariant deliberately narrowed.
  paws states it as "…in every deployment shape, WITHOUT a config file having to
  remember it", which is the sdk `create_cors_layer_with` UNION — unavailable here
  (see PLAN `## Out of scope`). ITEM-2 upholds the first clause only: the header the
  API reads IS now accepted by the preflight, in the desktop config and both shipped
  examples. Proven by MUTATION, not assertion: removing the entry makes the preflight
  test fail with `got "authorization,content-type,accept,origin,x-sync-connection-id"`
  — byte-identical to what a live upstream desktop instance returns. The
  "without a config file having to remember it" half is escalated with the sdk PR and
  is NOT claimed here.
- **INV-6** — fidelity: UPHELD, on both halves and at both ends. Server: the download
  SSE gets `keep_alive` (it was the only SSE route in the tree without it) and the
  LFS forwarder replaces an orphaned receiver, so the record actually advances during
  a multi-GB pull. Client: `progress_data` is rebuilt field-by-field instead of a flat
  spread behind an `as DownloadInstance` cast, so what a VIEW renders advances —
  which is what the invariant is about ("delivery to the UI failing while the
  underlying operation succeeds"), and what the previous attempt got wrong by
  asserting the server's write.
- **INV-7** — fidelity: UPHELD — the expectation now sorts by `(order, name)`, the
  same key `create_modules` uses, so it no longer depends on linker order.

**No `DROPPED` verdicts.** INV-5 is the only narrowing, it is stated rather than
quietly reframed, and the narrowed part is escalated with a named owner decision
attached.

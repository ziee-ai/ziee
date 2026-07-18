# Chunk D-full — DRIFT-1 (convergence loop)

Drift = any observable behaviour/wire/build difference vs the BG-3 boundary that
is NOT an intended, equivalence-preserving relocation.

## Checks run

| # | Drift probe | Result |
|---|---|---|
| 1 | `cargo check -p ziee` | exit 0 (pre-existing warnings only) |
| 2 | `cargo check -p ziee-desktop` | exit 0 (pre-existing warnings only) |
| 3 | `cd sdk && cargo check --workspace` | exit 0 (tauri + decorum + harness compile) |
| 4 | E8 golden — `types.ui.ts` | BYTE-IDENTICAL |
| 5 | E8 golden — `types.desktop.ts` | BYTE-IDENTICAL |
| 6 | E8 golden — `openapi.ui.json` | CANONICALLY-EQUAL (`jq -S`) |
| 7 | E8 golden — `openapi.desktop.json` | CANONICALLY-EQUAL |
| 8 | Harness `ziee::` CODE refs | 0 (`grep -rn 'ziee::' sdk/desktop/harness/src \| grep -v '//'` empty) |
| 9 | ziee-desktop backend unit tests (`modules::backend::`) | 7 passed / 0 failed |
| 10 | pgvector submodule left untracked (not staged) | confirmed (`?? server/vendor/pgvector`) |

## Drifts found + resolved

- **DR-a (candidate, resolved):** the boot-success `tracing::info!("Backend
  server started successfully on {}", handle.addr)` moved from the app into the
  harness `spawn_boot_then_window`. Verified it fires at the **same point**
  (immediately after `Ok(handle)`, before the app's `on_ready` stashes jwt/pool),
  with the identical format string → no observable log-order drift. Not a real
  drift.
- **DR-b (candidate, resolved):** `WindowConfig` field values vs the former window
  literals. Cross-checked each: `title ""`, `inner_size (1200,800)`,
  `min_inner_size (400,600)`, `effect_radius 8.0`, `traffic_light (20,22)` — all
  equal the originals. `config.title.as_str()` on an empty `String` == `""`. Not
  a real drift.
- **DR-c (candidate, resolved):** seam call shape changed from
  `ServerBoot::boot(&concrete)` to `Arc<dyn ServerBoot>::boot()`. Same dynamic
  dispatch to the same `ZieeServerBoot` impl; no behaviour change. Not a real
  drift.

## Wire-safety

The move relocates no aide/schemars handler or DTO — `WindowConfig` /
`create_main_window` / `spawn_boot_then_window` carry no `JsonSchema` and never
enter the OpenAPI surface. Hence both surfaces' `types.ts` are byte-identical and
`openapi.json` canonically-equal (checks 4–7). Had any moved type touched the
wire and drifted, the task STOP rule would apply — it did not.

**Unresolved drifts:** 0

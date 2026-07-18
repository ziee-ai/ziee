# Chunk `health` — TESTS-MOVED

## Moved INTO `ziee-health` (with `handlers.rs`, verbatim)

`crates/ziee-health/src/handlers.rs` `#[cfg(test)]` (5):

| Test | Covers |
|---|---|
| `health_check_returns_ok_200` | handler yields `200 OK` |
| `health_check_returns_200_and_ok_status` | `200` + `status == "ok"` |
| `health_check_returns_ok_status` | handler yields `status == "ok"` |
| `health_response_serde_round_trips_to_status_object` | `{"status":"ok"}` serialize + round-trip |
| `health_response_serializes_to_status_field` | `HealthResponse` → `status` field |

SDK result: `cargo test -p ziee-health` → **5 passed**.

## Stayed in ziee (app-coupled — not moved)

The HTTP-level integration test `tests/health/mod.rs::
health_endpoint_returns_ok_without_auth` stays in ziee's integration suite (it
drives the real router through the app's `TestServer`, which names `module_api`).
It is unaffected — the endpoint, route, and body are byte-identical.

No behavioral assertion was edited (only test relocation) — the
MOVE-preserves-behavior discipline holds.

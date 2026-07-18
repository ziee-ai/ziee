# Chunk C1 — TESTS-MOVED

## Moved INTO `ziee-control-mcp` (with the code)

### `crates/ziee-control-mcp/src/catalog.rs` `#[cfg(test)]` (7, verbatim)

| Test | Covers |
|---|---|
| `parses_operations_by_id` | operationId → method/path/summary/tags parse |
| `extracts_required_permission` | `**Required Permission:** \`x::y\`` parse |
| `no_permission_in_description_is_none` | no-marker → `None` |
| `json_body_flag_distinguishes_multipart` | JSON body vs multipart vs no-body |
| `path_params_extracted` | `{name}` extraction (incl. multi-param) |
| `detects_secret_request_field` | direct/$ref/nested/array/nullable-anyOf secret detection + false-positive guards |
| `parse_permission_edge_cases` | `a::b::c` / no-marker / `(none)` |

### `crates/ziee-control-mcp/src/policy.rs` `#[cfg(test)]` (3, verbatim)

| Test | Covers |
|---|---|
| `is_mutating_only_non_get` | GET = read, all else = mutating |
| `denylist_covers_every_category` | every deny class (auth/test/health/server-update/mcp-recursion/byte-stream/SSE/token-mint/secret-body/multipart) |
| `normal_operations_are_allowed` + `segment_matching_does_not_overreach` | drivable ops stay drivable; segment matching doesn't hide `mcp-servers`/`downloads`/`/api/mcp/*`/POST-download-actions |

SDK result: `cargo test -p ziee-control-mcp` → **11 passed**.

## Moved INTO `ziee-framework::mcp` (with the JSON-RPC scaffolding)

Out of `code_sandbox/types.rs` (5) + `code_sandbox/mod.rs` (2):

| Test | Covers |
|---|---|
| `jsonrpc_request_round_trip` | envelope deserialize |
| `jsonrpc_request_accepts_missing_jsonrpc_field` | serde default `2.0` |
| `jsonrpc_request_accepts_string_id` | string id |
| `jsonrpc_error_helpers_have_canonical_codes` | -32601/-32602/-32603 |
| `jsonrpc_response_serializes_with_either_result_or_error` | result-xor-error skip |
| `loopback_host_always_127_0_0_1_for_wildcards` | wildcard/empty → loopback |
| `loopback_host_pins_to_loopback_regardless_of_server_host` | SECURITY: attacker.com/IMDS/etc → loopback |

SDK result: `cargo test -p ziee-framework mcp::` → **7 passed**.

## Stayed in ziee (app-coupled — not moved)

| Test home | Why it stayed |
|---|---|
| `control_mcp/handlers.rs` `#[cfg(test)]` (validate_body×4, substitute_path×2, is_path_safe, needs_approval_decision×3) | The handler stays app-side (v1); its tests exercise `super::catalog`/`super::policy` through the shim + the app-side request hardening + approval classifier. |
| `code_sandbox/types.rs` (resolve_flavor_lock, conversation_flavor_lock_pins_then_switches, conversation_id_header_×3) | Sandbox-specific, unrelated to the JSON-RPC types. |
| `code_sandbox/mod.rs` (`code_sandbox_server_id_is_stable`, …) | Sandbox-specific; only the 2 loopback tests moved. |

No behavioral assertion was edited (only import-path / test-relocation) — the
MOVE-preserves-behavior discipline holds.

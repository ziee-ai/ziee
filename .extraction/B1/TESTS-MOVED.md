# Chunk B1 — TESTS-MOVED (coverage-preservation)

Every ziee test that covered the moved code, and where it now runs. Ported tests
PASS in `cargo test -p ziee-core`; retained tests compile green in the ziee lib
test target (`cargo test -p ziee --lib --no-run`).

- **T-B1-1** [ported→sdk] file: `sdk/crates/ziee-core/src/error.rs` covers: AppError::database_error redaction (database_error_does_not_leak_inner_error_display)
- **T-B1-2** [ported→sdk] file: `sdk/crates/ziee-core/src/error.rs` covers: AppError::internal_with_id redaction (internal_with_id_does_not_leak_inner_error_display)
- **T-B1-3** [ported→sdk] file: `sdk/crates/ziee-core/src/error.rs` covers: From<sqlx::Error> for AppError redaction (from_sqlx_error_does_not_leak_inner_error_display)
- **T-B1-4** [ported→sdk] file: `sdk/crates/ziee-core/src/error.rs` covers: From<Box<dyn Error>> for AppError redaction (from_boxed_error_does_not_leak_inner_error_display)
- **T-B1-5** [ported→sdk] file: `sdk/crates/ziee-core/src/error.rs` covers: AppError::database_error trace_id correlation (database_error_includes_trace_id_for_correlation)
- **T-B1-6** [ported→sdk] file: `sdk/crates/ziee-core/src/error.rs` covers: AppError::not_found safe-constructor (not_found_does_not_route_through_redaction)
- **T-B1-7** [ported→sdk] file: `sdk/crates/ziee-core/src/app_state.rs` covers: APP_DATA_DIR set/get round-trip (test_app_data_dir)
- **T-B1-8** [stays→ziee] file: `src-app/server/src/common/type.rs` covers: PaginationQuery deserialize-clamp (pagination_clamps_* + passes_through + defaults, 6 tests — PaginationQuery stays app-side, not in B1 scope)
- **T-B1-9** [stays→ziee] file: `src-app/server/src/core/app_state.rs` covers: MAX_FILE_UPLOAD_BYTES round-trip + derived body limit (max_file_upload_bytes_round_trip_and_derived_body_limit — MAX_FILE_UPLOAD stays app-side)
- **T-B1-10** [stays→ziee] file: `src-app/server/src/core/app_state.rs` covers: docker/nginx upload-cap plumbing (docker_web_plumbs_max_file_upload_var + nginx_body_size_covers_default_body_limit — read ziee-relative docker files, must stay app-side)

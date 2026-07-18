# Chunk B2 — TESTS-MOVED (coverage-preservation)

Every ziee test that covered the moved code, and where it now runs. Ported tests
PASS in `cargo test -p ziee-core` (13 passed, 0 failed); retained tests stay in
ziee and continue to exercise the module-registration path against ziee's live
modules.

Config sub-type tests (moved verbatim WITH their types from `core/config.rs`):

- **T-B2-1** [ported→sdk] file: `sdk/crates/ziee-core/src/config.rs` covers: RateLimitConfig serde-default branches — enabled defaults true when omitted, disable via just `enabled:false`, full-block parse (rate_limit_config_tests: enabled_defaults_true_when_field_omitted, can_disable_with_just_enabled_flag, full_block_parses_all_fields)
- **T-B2-2** [ported→sdk] file: `sdk/crates/ziee-core/src/config.rs` covers: HttpServerConfig max_file_upload_mb default(128) + explicit override + the default_max_file_upload_mb fn (max_file_upload_tests: default_is_128, omitted_key_deserializes_to_default, explicit_key_overrides_default)

Config tests that STAY in ziee (cover domain types / the composed Config, which stay):

- **T-B2-3** [stays→ziee] file: `src-app/server/src/core/config.rs` covers: VoiceConfig deploy-kill-switch resolution (voice_config_tests) — domain type, stays.
- **T-B2-4** [stays→ziee] file: `src-app/server/src/core/config.rs` covers: CodeSandboxConfig::public_file_origin (public_file_origin_tests) — domain type, stays.
- **T-B2-5** [stays→ziee] file: `src-app/server/src/core/config.rs` covers: the packaged default config parses as a full `Config` (packaging_config_tests::packaged_default_config_parses) — this is now ALSO the equivalence check that `#[serde(flatten)]` keeps the full-YAML wire shape parseable; stays in ziee (parses the composed `Config`).

Module-registration tests that STAY in ziee (need ziee's live `MODULE_ENTRIES`):

- **T-B2-6** [stays→ziee] file: `src-app/server/src/core/app_builder.rs` covers: create_modules instantiates every registered entry in ascending order (create_modules_instantiates_all_entries_in_order) — RETAINED in the ziee shim because it needs ziee's 44 modules linked into the framework's `MODULE_ENTRIES` slice; it is the executable proof that cross-crate linkme registration works.
- **T-B2-7** [stays→ziee] file: `src-app/server/src/core/app_builder.rs` covers: module names are unique (module_names_are_unique) — RETAINED for the same reason.

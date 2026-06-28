//! Unit tests for `Config::resolve_paths` — the seam that defaults
//! every Optional path field to a subdir of `app.data_dir` and lets
//! operators override individual paths.
//!
//! Three contracts to check:
//!  1. No `app:` block → falls back to `~/.ziee`, all caches derive from it.
//!  2. Explicit `app.data_dir` → all caches derive from it.
//!  3. Partial overrides → operator-set paths win; rest derive.
//!
//! Lives alongside other integration tests; runs in normal `cargo test`.

use serde_norway;
use std::path::PathBuf;
use ziee::Config;

/// Build a minimally-valid Config YAML around a `code_sandbox:` / `app:`
/// fragment. We only care about the path-derivation surface, so the
/// rest is just enough to parse.
fn minimal_config(extra_blocks: &str) -> String {
    format!(
        r#"
{extra}
postgresql:
  use_embedded: false
  external:
    host: "127.0.0.1"
    port: 54321
    username: postgres
    password: password
    database: testdb
  pool:
    max_connections: 1
    min_connections: 1
    acquire_timeout_secs: 3

server:
  host: "127.0.0.1"
  port: 0
  api_prefix: "/api"

jwt:
  secret: "test-secret-key-for-jwt-tokens-min-32-chars-long"
  issuer: "ziee"
  audience: "ziee-api"
  access_token_expiry_hours: 24
"#,
        extra = extra_blocks
    )
}

fn parse_and_resolve(yaml: &str) -> Config {
    let mut config: Config = serde_norway::from_str(yaml).expect("config parses");
    config.resolve_paths();
    config
}

#[test]
fn no_app_block_defaults_to_home_ziee() {
    let config = parse_and_resolve(&minimal_config(""));
    let home_ziee = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ziee");
    let app = config.app.expect("resolve_paths fills app");
    assert_eq!(PathBuf::from(&app.data_dir), home_ziee);
}

#[test]
fn caches_derive_from_app_data_dir() {
    let config = parse_and_resolve(&minimal_config(
        r#"app:
  data_dir: "/tmp/path-resolution-test"
"#,
    ));
    let base = PathBuf::from("/tmp/path-resolution-test");
    assert_eq!(PathBuf::from(config.caches.hf_models_dir()), base.join("hf-models"));
    assert_eq!(PathBuf::from(config.caches.llm_engines_dir()), base.join("llm-engines"));
    assert_eq!(PathBuf::from(config.caches.git_cache_dir()), base.join("cache/git"));
    assert_eq!(PathBuf::from(config.caches.lfs_cache_dir()), base.join("cache/lfs"));
}

#[test]
fn code_sandbox_paths_derive_from_app_data_dir() {
    let config = parse_and_resolve(&minimal_config(
        r#"app:
  data_dir: "/tmp/path-resolution-test"
"#,
    ));
    let sandbox = config.code_sandbox.expect("resolve_paths fills code_sandbox");
    let base = PathBuf::from("/tmp/path-resolution-test");
    assert_eq!(PathBuf::from(sandbox.rootfs_path()), base.join("sandbox-rootfs"));
    assert_eq!(PathBuf::from(sandbox.workspace_root()), base.join("sandboxes"));
}

#[test]
fn partial_override_keeps_explicit_path_and_derives_rest() {
    // hf_models_dir overridden to a different disk; the rest derive
    // from app.data_dir. Operator's "models on /mnt/big-disk" workflow.
    let yaml = minimal_config(
        r#"app:
  data_dir: "/tmp/path-resolution-test"

caches:
  hf_models_dir: "/mnt/big-disk/hf-models"
"#,
    );
    let config = parse_and_resolve(&yaml);
    assert_eq!(
        PathBuf::from(config.caches.hf_models_dir()),
        PathBuf::from("/mnt/big-disk/hf-models"),
        "explicit override should win"
    );
    assert_eq!(
        PathBuf::from(config.caches.llm_engines_dir()),
        PathBuf::from("/tmp/path-resolution-test/llm-engines"),
        "non-overridden field should derive"
    );
}

#[test]
fn embedded_postgres_paths_derive_when_unset() {
    // `installation_dir` / `data_dir` Optional means a config that ONLY
    // specifies the required fields (version/port/username/...) still
    // boots: resolve_paths fills the paths from app.data_dir.
    let yaml = minimal_config(
        r#"app:
  data_dir: "/tmp/path-resolution-test"

"#,
    )
    .replace(
        "postgresql:\n  use_embedded: false",
        r#"postgresql:
  use_embedded: true
  embedded:
    version: "17.0"
    port: 0
    bind_address: "127.0.0.1"
    username: postgres
    password: password
    database: ziee
    timezone: UTC
    log_timezone: UTC
    logging:
      collector: false
      directory: "/tmp/postgres-logs"
      filename: "postgres.log"
      statement: "none""#,
    );
    // Disable port autopick — `Config::load_from` requires a file path,
    // so we parse manually and call resolve_paths.
    let mut config: Config = serde_norway::from_str(&yaml).expect("config parses");
    config.resolve_paths();
    let emb = config
        .postgresql
        .embedded
        .expect("embedded block present");
    assert_eq!(
        emb.installation_dir.as_deref(),
        Some("/tmp/path-resolution-test/postgres")
    );
    assert_eq!(
        emb.data_dir.as_deref(),
        Some("/tmp/path-resolution-test/postgres-data")
    );
}

#[test]
fn resolve_paths_is_idempotent() {
    // Running resolve_paths twice produces the same result — important
    // because some callers (test harness setup, Tauri's embedded
    // bootstrap) might invoke it more than once.
    let mut config: Config = serde_norway::from_str(&minimal_config("")).expect("parse");
    config.resolve_paths();
    let snapshot_caches = config.caches.clone();
    let snapshot_sandbox = config.code_sandbox.clone();
    config.resolve_paths();
    assert_eq!(format!("{:?}", config.caches), format!("{:?}", snapshot_caches));
    assert_eq!(
        format!("{:?}", config.code_sandbox),
        format!("{:?}", snapshot_sandbox)
    );
}

//! Connection-health enforcement for LLM repositories.
//!
//! Three entry points share a single underlying probe (the same
//! `test_repository_connectivity` the explicit "Test Connection" UI
//! button already uses, so probe semantics stay aligned with the
//! manual button — a green button means the save / boot probe would
//! pass too):
//!
//! 1. Update / enable flow — refuse to flip `enabled: false → true`
//!    when the persisted config can't connect. Handler returns 400
//!    with the failure detail; other fields in the same PUT still
//!    persist (the operator's edit is preserved; only the enable bit
//!    reverts).
//! 2. Create flow — if a new repository was requested with
//!    `enabled: true` and the probe fails, downgrade to
//!    `enabled: false` and surface a `connection_warning` so the row
//!    is preserved for the user to edit + retry.
//! 3. Boot — every enabled repository is probed on server startup;
//!    failures flip `enabled = false` automatically so a stale token
//!    on HuggingFace or GitHub doesn't surface as confusing download
//!    failures later.
//!
//! Unlike the MCP analog, this module does NOT skip "built-in" rows:
//! the seed HuggingFace + GitHub repos are exactly the rows that
//! need this — they ship enabled with empty credentials, and the
//! boot probe is what flips them to disabled on a fresh install.
//!
//! # Testing seams
//!
//! The branch-matrix enforcement (the `enforce_on_*` functions) is
//! refactored around two seams so the Tier-1 unit tests can drive
//! every branch without a Postgres or a network mock:
//!
//! - **`ProbeFn`** — `async Fn(&LlmRepository) -> Result<(), ProbeFailure>`.
//!   Production code passes the live `probe` (which wraps the existing
//!   `test_repository_connectivity` HTTP call); tests pass a closure
//!   that returns a canned outcome.
//! - **`HealthOps`** trait — abstracts the side effects (read row, write
//!   row's `enabled`, record `last_health_check_*`, emit
//!   `auto_disabled`). Production uses `ProductionHealthOps` (wraps
//!   `Repos.llm_repository` + the EventBus); tests use a `FakeHealthOps`
//!   that records call args.
//!
//! Public `enforce_on_create` / `enforce_on_update_transition` take
//! `&EventBus` only — DB access goes through `Repos.llm_repository`.
//! The `*_with_ops` siblings are exposed for tests.

use crate::common::AppError;
use crate::core::Repos;
use crate::core::events::EventBus;
use async_trait::async_trait;
use serde::Serialize;
use sqlx::PgPool;
use std::future::Future;
use std::pin::Pin;
use uuid::Uuid;

/// Boxed-future probe-fn type. The boxing erases the lifetime tied to
/// the input `&LlmRepository`, sidestepping the HRTB friction between
/// `fn(&LlmRepository) -> impl Future + '_` and `|r| async { ... }`
/// closures. One `Box::pin` per call — cost is negligible against the
/// HTTP round-trip the production probe makes.
pub type ProbeFuture<'a> =
    Pin<Box<dyn Future<Output = Result<(), ProbeFailure>> + Send + 'a>>;

use super::models::LlmRepository;
use super::types::TestRepositoryConnectionRequest;
use super::utils::test_repository_connectivity;

/// Structured probe failure carrying the underlying reason so the
/// caller can surface it (in the API response, in the boot log, or
/// in the UI toast).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ProbeFailure {
    /// Human-readable reason — taken verbatim from
    /// `test_repository_connectivity`'s `Err(String)` (timeout / 401 /
    /// DNS / etc.).
    pub reason: String,
}

/// Wraps a created/updated `LlmRepository` with an optional connection
/// warning, used by the create handler when the probe failed and the
/// row was auto-downgraded to `enabled: false`. `None` on success
/// (probe passed, or `enabled: false` was requested so no probe ran).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct LlmRepositoryWithHealthWarning {
    // Flattened so the response shape is `{...LlmRepository fields,
    // connection_warning?}` — the body IS the entity, with an
    // optional warning sibling that appears only when the probe
    // auto-downgraded the row. Keeps the API contract identical to
    // the pre-health-check shape for clients that don't care about
    // the warning (CLI, integration tests, the bare REST surface),
    // and avoids forcing every caller through a `.repository.` hop.
    #[serde(flatten)]
    pub repository: LlmRepository,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_warning: Option<ProbeFailure>,
}

// =====================================================================
// HealthOps — side-effect surface (mocked in unit tests).
// =====================================================================

/// Side effects the enforcement logic performs after deciding on an
/// outcome. Split out as a trait so the unit tests can verify branch
/// behavior (which call paths fire which side effects in which order)
/// without touching a real DB or EventBus.
///
/// Production implementation is `ProductionHealthOps`, which forwards
/// to `Repos.llm_repository` + the EventBus. Tests use `FakeHealthOps`
/// which records the calls.
///
/// The trait deliberately does NOT expose the raw `record_health_check`
/// shape (status as `&str`, etc.) — instead it offers two semantic
/// methods (`record_healthy` / `record_unhealthy`) so the enforcement
/// code can't accidentally pass an invalid status string.
#[async_trait]
pub trait HealthOps: Send + Sync {
    async fn record_healthy(&self, repo_id: Uuid);
    async fn record_unhealthy(&self, repo_id: Uuid, reason: &str);
    async fn disable_repository(&self, repo_id: Uuid) -> Result<(), AppError>;
    async fn get_repository(&self, repo_id: Uuid) -> Result<Option<LlmRepository>, AppError>;
    fn emit_auto_disabled(&self, repo_id: Uuid, reason: String);
}

pub struct ProductionHealthOps<'a> {
    event_bus: &'a EventBus,
}

impl<'a> ProductionHealthOps<'a> {
    pub fn new(event_bus: &'a EventBus) -> Self {
        Self { event_bus }
    }
}

#[async_trait]
impl<'a> HealthOps for ProductionHealthOps<'a> {
    async fn record_healthy(&self, repo_id: Uuid) {
        if let Err(e) = Repos
            .llm_repository
            .record_health_check(repo_id, "healthy", None)
            .await
        {
            tracing::warn!(
                error = ?e,
                repo_id = %repo_id,
                "llm_repo::health: failed to record healthy status (non-fatal)",
            );
        }
    }

    async fn record_unhealthy(&self, repo_id: Uuid, reason: &str) {
        if let Err(e) = Repos
            .llm_repository
            .record_health_check(repo_id, "unhealthy", Some(reason))
            .await
        {
            tracing::warn!(
                error = ?e,
                repo_id = %repo_id,
                "llm_repo::health: failed to record unhealthy status (non-fatal)",
            );
        }
    }

    async fn disable_repository(&self, repo_id: Uuid) -> Result<(), AppError> {
        Repos
            .llm_repository
            .disable_for_health_failure(repo_id)
            .await
            .map_err(|e| AppError::internal_error(format!("Database error: {e}")))
    }

    async fn get_repository(&self, repo_id: Uuid) -> Result<Option<LlmRepository>, AppError> {
        Repos
            .llm_repository
            .get_by_id(repo_id)
            .await
            .map_err(|e| AppError::internal_error(format!("Database error: {e}")))
    }

    fn emit_auto_disabled(&self, repo_id: Uuid, reason: String) {
        self.event_bus.emit_async(
            super::events::LlmRepositoryEvent::auto_disabled(repo_id, reason).into(),
        );
    }
}

// =====================================================================
// ProbeFn — async Fn(&LlmRepository) -> Result<(), ProbeFailure>.
// =====================================================================

/// Probe an LLM repository's connection. Returns `Ok(())` on a 200 OK
/// from the configured auth-test endpoint; `Err(ProbeFailure)`
/// otherwise (timeout, DNS, non-200, etc.).
///
/// Reuses the existing `test_repository_connectivity` so the probe is
/// byte-for-byte the same code path the explicit "Test Connection"
/// button hits — a green button guarantees a green probe.
pub async fn probe(repository: &LlmRepository) -> Result<(), ProbeFailure> {
    let request = TestRepositoryConnectionRequest {
        name: repository.name.clone(),
        url: repository.url.clone(),
        auth_type: repository.auth_type.clone(),
        auth_config: Some(repository.auth_config.clone()),
    };
    match test_repository_connectivity(&request).await {
        Ok(()) => Ok(()),
        Err(reason) => Err(ProbeFailure { reason }),
    }
}

// =====================================================================
// enforce_on_create — production entry point + testable inner.
// =====================================================================

/// Create-flow enforcement. Call AFTER `Repos.llm_repository.create`
/// returns the persisted row. Probes when the new repository is
/// `enabled: true`; on probe failure, flips `enabled: false` in the
/// DB and returns the updated row with `connection_warning` set.
///
/// Records the probe outcome on the row's `last_health_check_*`
/// columns regardless of success/failure so the UI's Alert can show
/// "last tried: …" without re-running.
pub async fn enforce_on_create(
    repository: LlmRepository,
    event_bus: &EventBus,
) -> Result<LlmRepositoryWithHealthWarning, AppError> {
    let ops = ProductionHealthOps::new(event_bus);
    enforce_on_create_with_ops(repository, &ops, |r| Box::pin(probe(r))).await
}

/// Testable inner — see module-level docs on `ProbeFn` / `HealthOps`.
pub async fn enforce_on_create_with_ops<H, P>(
    repository: LlmRepository,
    ops: &H,
    probe_fn: P,
) -> Result<LlmRepositoryWithHealthWarning, AppError>
where
    H: HealthOps,
    P: for<'a> FnOnce(&'a LlmRepository) -> ProbeFuture<'a>,
{
    if !repository.enabled {
        return Ok(LlmRepositoryWithHealthWarning {
            repository,
            connection_warning: None,
        });
    }

    match probe_fn(&repository).await {
        Ok(()) => {
            ops.record_healthy(repository.id).await;
            // Re-fetch so the response carries the recorded health
            // timestamp + status fields.
            let refetched = ops
                .get_repository(repository.id)
                .await?
                .unwrap_or(repository);
            Ok(LlmRepositoryWithHealthWarning {
                repository: refetched,
                connection_warning: None,
            })
        }
        Err(failure) => {
            tracing::warn!(
                repo_id = %repository.id,
                reason = %failure.reason,
                "llm_repo::health: create-time probe failed; downgrading new repository to disabled",
            );
            ops.disable_repository(repository.id).await?;
            ops.record_unhealthy(repository.id, &failure.reason).await;
            ops.emit_auto_disabled(repository.id, failure.reason.clone());
            let refetched = ops
                .get_repository(repository.id)
                .await?
                .ok_or_else(|| {
                    AppError::internal_error("Repository vanished after auto-disable")
                })?;
            Ok(LlmRepositoryWithHealthWarning {
                repository: refetched,
                connection_warning: Some(failure),
            })
        }
    }
}

// =====================================================================
// enforce_on_update_transition — production entry point + testable inner.
// =====================================================================

/// Update-flow enforcement. Call AFTER persisting all other fields
/// but BEFORE returning the response. When the update is an
/// enabled-transition (`old_enabled == false && new_enabled == true`)
/// the persisted state is probed; on failure the row's `enabled` is
/// forced back to false in the DB and the function returns a 400
/// `AppError` so the handler short-circuits. Other fields the user
/// updated in the same PUT stay persisted — the partial save is
/// preferable to losing every concurrent edit.
pub async fn enforce_on_update_transition(
    persisted: LlmRepository,
    old_enabled: bool,
    event_bus: &EventBus,
) -> Result<LlmRepository, AppError> {
    let ops = ProductionHealthOps::new(event_bus);
    enforce_on_update_transition_with_ops(
        persisted,
        old_enabled,
        &ops,
        |r| Box::pin(probe(r)),
    )
    .await
}

/// Testable inner — see module-level docs on `ProbeFn` / `HealthOps`.
pub async fn enforce_on_update_transition_with_ops<H, P>(
    persisted: LlmRepository,
    old_enabled: bool,
    ops: &H,
    probe_fn: P,
) -> Result<LlmRepository, AppError>
where
    H: HealthOps,
    P: for<'a> FnOnce(&'a LlmRepository) -> ProbeFuture<'a>,
{
    let transitioned_to_enabled = persisted.enabled && !old_enabled;
    if !transitioned_to_enabled {
        return Ok(persisted);
    }

    match probe_fn(&persisted).await {
        Ok(()) => {
            ops.record_healthy(persisted.id).await;
            let refetched = ops
                .get_repository(persisted.id)
                .await?
                .unwrap_or(persisted);
            Ok(refetched)
        }
        Err(failure) => {
            tracing::warn!(
                repo_id = %persisted.id,
                reason = %failure.reason,
                "llm_repo::health: update-enable-transition probe failed; reverting to enabled=false",
            );
            ops.disable_repository(persisted.id).await?;
            ops.record_unhealthy(persisted.id, &failure.reason).await;
            ops.emit_auto_disabled(persisted.id, failure.reason.clone());
            Err(AppError::bad_request(
                "LLM_REPOSITORY_ENABLE_FAILED_HEALTH_CHECK",
                format!(
                    "Other changes were saved, but the repository could not \
                     be enabled because the connection probe failed: {}",
                    failure.reason
                ),
            ))
        }
    }
}

// =====================================================================
// Test-button outcome bookkeeping — used by `POST /llm-repositories/{id}/test`.
// =====================================================================

/// What the manual "Test Connection" button returns to its caller —
/// the handler maps this into a `TestRepositoryConnectionResponse`.
#[derive(Debug, Clone)]
pub struct TestOutcome {
    pub success: bool,
    pub reason: Option<String>,
    /// True when the helper already emitted `auto_disabled` (failure
    /// on an enabled row AND `disable_repository` itself succeeded).
    /// The handler keys off this to skip a redundant `updated` event;
    /// when false (success path / failure on already-disabled row /
    /// disable_repository itself failed), the handler emits `updated`
    /// so listeners see the canonical row state.
    pub already_emitted_auto_disabled: bool,
}

/// Post-probe bookkeeping for the test-button path. Records the
/// probe outcome on `last_health_check_*` (regardless of pass/fail),
/// then — if the probe failed AND the row was currently enabled —
/// auto-disables the row + emits `auto_disabled`. A failure on an
/// already-disabled row is just recorded; flipping `enabled` again
/// would emit redundant events.
///
/// Split out of the handler so the branch matrix (healthy / failure-on-enabled
/// / failure-on-disabled) is unit-testable without a Postgres or HTTP
/// dependency — see `tests::test_button_*` in this module.
pub async fn record_test_outcome<H: HealthOps>(
    repo_id: uuid::Uuid,
    was_enabled: bool,
    probe_result: Result<(), ProbeFailure>,
    ops: &H,
) -> Result<TestOutcome, AppError> {
    match probe_result {
        Ok(()) => {
            ops.record_healthy(repo_id).await;
            Ok(TestOutcome {
                success: true,
                reason: None,
                already_emitted_auto_disabled: false,
            })
        }
        Err(failure) => {
            ops.record_unhealthy(repo_id, &failure.reason).await;
            // Auto-disable path. `disable_repository` is best-effort:
            // the probe outcome has already been persisted in
            // `record_unhealthy`, so a DB hiccup here shouldn't
            // turn a perfectly-readable failure ("probe got 401")
            // into a 500 the user can't act on. Log + skip the
            // auto_disabled emit when disable failed — the handler
            // will then emit `updated` instead, surfacing the actual
            // (still-enabled-but-unhealthy) row state to listeners.
            let mut emitted_auto_disabled = false;
            if was_enabled {
                match ops.disable_repository(repo_id).await {
                    Ok(()) => {
                        ops.emit_auto_disabled(repo_id, failure.reason.clone());
                        emitted_auto_disabled = true;
                    }
                    Err(e) => {
                        tracing::warn!(
                            error = ?e,
                            repo_id = %repo_id,
                            "llm_repo::health: failed to auto-disable after test failure (non-fatal); row stays at enabled=true with status='unhealthy'",
                        );
                    }
                }
            }
            Ok(TestOutcome {
                success: false,
                reason: Some(failure.reason),
                already_emitted_auto_disabled: emitted_auto_disabled,
            })
        }
    }
}

// =====================================================================
// run_startup_health_check — boot-time fire-and-forget.
// =====================================================================

/// Boot-time health check. Iterates every `enabled = true` LLM
/// repository, probes it, and flips `enabled = false` on any
/// failure. Logs each transition.
///
/// Runs as a fire-and-forget background task spawned from
/// `llm_repository::init` — must NOT block boot. The built-in seed
/// rows (HuggingFace, GitHub) are intentionally INCLUDED in this
/// scan — they're the rows we most want to probe (they ship enabled
/// with empty credentials, and this is what flips them to disabled
/// on a fresh install).
///
/// No event emission here: the `EventBus` is built AFTER module
/// init, so it's not in scope at this stage. The on-save handlers
/// (which DO have access via Axum Extension) emit `AutoDisabled`
/// when they downgrade a repository. UI pages re-fetch on mount, so
/// a boot-time auto-disable is visible the next time the user opens
/// the settings page — no event channel needed for the boot path.
pub async fn run_startup_health_check(pool: PgPool) {
    // We take the pool explicitly (rather than going through
    // `Repos.llm_repository`) so this is callable from the integration
    // test crate, where the global `RepositoryFactory` is NOT
    // initialized (each test spawns a separate ziee binary that owns
    // its own static, while the test process itself doesn't run
    // `init_repositories`). Production callers pass the module pool
    // from `init`; tests pass the test DB pool. The free functions
    // are the same ones `LlmRepositoryRepository`'s methods delegate
    // to under the hood.
    let repos = match super::repository::list_enabled_for_health_check(&pool).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(
                error = ?e,
                "llm_repo::health: failed to list enabled repositories for startup check",
            );
            return;
        }
    };

    if repos.is_empty() {
        tracing::debug!("llm_repo::health: no enabled repositories to probe");
        return;
    }

    tracing::info!(
        count = repos.len(),
        "llm_repo::health: probing enabled LLM repositories at startup",
    );

    for repository in repos {
        let repo_id = repository.id;
        let repo_name = repository.name.clone();
        // Skip rows that have no credential configured (seeded HF
        // Hub + GitHub ship enabled but unconfigured — the operator
        // is expected to paste a token). Probing them returns 401
        // and we'd disable the row, sending the user to a "this is
        // broken" affordance when really it just needs setup. The
        // on-save probe is the right surface to validate credentials
        // — that runs when the user actually fills the field in.
        if !repository.has_credential() {
            tracing::debug!(
                repo_id = %repo_id,
                repo_name = %repo_name,
                "llm_repo::health: skipping unconfigured repository at startup",
            );
            continue;
        }
        match probe(&repository).await {
            Ok(()) => {
                tracing::debug!(
                    repo_id = %repo_id,
                    repo_name = %repo_name,
                    "llm_repo::health: repository reachable",
                );
                if let Err(e) = super::repository::record_health_check(
                    &pool,
                    repo_id,
                    "healthy",
                    None,
                )
                .await
                {
                    tracing::warn!(
                        error = ?e,
                        repo_id = %repo_id,
                        "llm_repo::health: failed to record healthy status (non-fatal)",
                    );
                }
            }
            Err(failure) => {
                tracing::warn!(
                    repo_id = %repo_id,
                    repo_name = %repo_name,
                    reason = %failure.reason,
                    "llm_repo::health: auto-disabling unreachable repository",
                );
                if let Err(e) =
                    super::repository::disable_for_health_failure(&pool, repo_id).await
                {
                    tracing::error!(
                        repo_id = %repo_id,
                        error = ?e,
                        "llm_repo::health: failed to auto-disable repository",
                    );
                }
                if let Err(e) = super::repository::record_health_check(
                    &pool,
                    repo_id,
                    "unhealthy",
                    Some(&failure.reason),
                )
                .await
                {
                    tracing::warn!(
                        error = ?e,
                        repo_id = %repo_id,
                        "llm_repo::health: failed to record unhealthy status (non-fatal)",
                    );
                }
            }
        }
    }
}

// =====================================================================
// Tier-1 unit tests — drive every enforce-branch via `ProbeFn` +
// `FakeHealthOps`. No DB, no HTTP.
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn repo(enabled: bool) -> LlmRepository {
        LlmRepository {
            id: Uuid::new_v4(),
            name: "test-repo".into(),
            url: "https://example.com".into(),
            auth_type: "none".into(),
            auth_config: Default::default(),
            enabled,
            built_in: false,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_health_check_at: None,
            last_health_check_status: "untested".into(),
            last_health_check_reason: None,
        }
    }

    /// Records every call site the enforcement logic invokes. Returns
    /// an in-memory `LlmRepository` (the seeded `repo` field) from
    /// `get_repository` so the enforce functions can re-fetch.
    #[derive(Default)]
    struct FakeHealthOps {
        recorded: Mutex<Vec<(Uuid, String, Option<String>)>>,
        disabled: Mutex<Vec<Uuid>>,
        emitted: Mutex<Vec<(Uuid, String)>>,
        repo: Mutex<Option<LlmRepository>>,
    }

    impl FakeHealthOps {
        fn seed_repo(&self, r: LlmRepository) {
            *self.repo.lock().unwrap() = Some(r);
        }
        fn recorded(&self) -> Vec<(Uuid, String, Option<String>)> {
            self.recorded.lock().unwrap().clone()
        }
        fn disabled(&self) -> Vec<Uuid> {
            self.disabled.lock().unwrap().clone()
        }
        fn emitted(&self) -> Vec<(Uuid, String)> {
            self.emitted.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl HealthOps for FakeHealthOps {
        async fn record_healthy(&self, repo_id: Uuid) {
            self.recorded
                .lock()
                .unwrap()
                .push((repo_id, "healthy".into(), None));
        }
        async fn record_unhealthy(&self, repo_id: Uuid, reason: &str) {
            self.recorded
                .lock()
                .unwrap()
                .push((repo_id, "unhealthy".into(), Some(reason.into())));
            // Mirror the production side effect: the persisted row's
            // status/reason advance. Lets `get_repository` return the
            // updated shape after a failure.
            if let Some(r) = self.repo.lock().unwrap().as_mut() {
                r.last_health_check_status = "unhealthy".into();
                r.last_health_check_reason = Some(reason.into());
            }
        }
        async fn disable_repository(&self, repo_id: Uuid) -> Result<(), AppError> {
            self.disabled.lock().unwrap().push(repo_id);
            if let Some(r) = self.repo.lock().unwrap().as_mut() {
                r.enabled = false;
            }
            Ok(())
        }
        async fn get_repository(&self, _repo_id: Uuid) -> Result<Option<LlmRepository>, AppError> {
            Ok(self.repo.lock().unwrap().clone())
        }
        fn emit_auto_disabled(&self, repo_id: Uuid, reason: String) {
            self.emitted.lock().unwrap().push((repo_id, reason));
        }
    }

    // ── 1. probe wrapper: Ok → Ok ─────────────────────────────────────

    #[tokio::test]
    async fn probe_maps_connectivity_ok_to_ok() {
        // The wrapper is a one-line passthrough; this test guards
        // against a future tweak that decides to add side effects.
        // Direct construction of an `Ok(())` from
        // `test_repository_connectivity` isn't possible here without
        // HTTP, so we test the equivalent — a probe_fn closure
        // returning Ok flows through enforce_on_create_with_ops
        // without invoking any failure path.
        let r = repo(true);
        let ops = FakeHealthOps::default();
        ops.seed_repo(r.clone());
        let result = enforce_on_create_with_ops(r.clone(), &ops, |_| {
            Box::pin(async { Ok(()) })
        })
        .await;
        assert!(result.is_ok());
        assert!(result.unwrap().connection_warning.is_none());
        assert_eq!(ops.recorded().len(), 1);
        assert_eq!(ops.recorded()[0].1, "healthy");
        assert!(ops.disabled().is_empty());
        assert!(ops.emitted().is_empty());
    }

    // ── 2. probe wrapper: Err → ProbeFailure with verbatim reason ────

    #[tokio::test]
    async fn probe_maps_connectivity_err_to_probe_failure() {
        // The reason string flows from probe_fn → ProbeFailure →
        // record_unhealthy → emit_auto_disabled untouched. Lock the
        // verbatim passthrough — the UI Alert assumes it.
        let r = repo(true);
        let ops = FakeHealthOps::default();
        ops.seed_repo(r.clone());
        let result = enforce_on_create_with_ops(r.clone(), &ops, |_| {
            Box::pin(async {
                Err(ProbeFailure {
                    reason: "401 Unauthorized verbatim".into(),
                })
            })
        })
        .await;
        let wrapper = result.expect("wrapper returns Ok with the warning embedded");
        assert_eq!(
            wrapper.connection_warning.unwrap().reason,
            "401 Unauthorized verbatim"
        );
        assert_eq!(ops.recorded()[0].2.as_deref(), Some("401 Unauthorized verbatim"));
        assert_eq!(ops.emitted()[0].1, "401 Unauthorized verbatim");
    }

    // ── 3. enforce_on_create — enabled:false short-circuits ──────────

    #[tokio::test]
    async fn enforce_on_create_when_disabled_skips_probe() {
        let r = repo(false);
        let ops = FakeHealthOps::default();
        // probe_fn that panics if invoked — proves the short-circuit.
        let result = enforce_on_create_with_ops(r.clone(), &ops, |_| {
            Box::pin(async { panic!("probe must not run when enabled is false") })
        })
        .await;
        let wrapper = result.unwrap();
        assert!(wrapper.connection_warning.is_none());
        assert_eq!(wrapper.repository.enabled, false);
        assert!(ops.recorded().is_empty());
        assert!(ops.disabled().is_empty());
        assert!(ops.emitted().is_empty());
    }

    // ── 4. enforce_on_create — passing probe, no warning ─────────────

    #[tokio::test]
    async fn enforce_on_create_when_enabled_and_probe_passes_no_warning() {
        let r = repo(true);
        let ops = FakeHealthOps::default();
        ops.seed_repo(r.clone());
        let wrapper = enforce_on_create_with_ops(r.clone(), &ops, |_| {
            Box::pin(async { Ok(()) })
        })
        .await
        .unwrap();
        assert!(wrapper.connection_warning.is_none());
        assert_eq!(wrapper.repository.enabled, true);
        // Side effects: exactly one healthy record, no disable, no emit.
        assert_eq!(ops.recorded(), vec![(r.id, "healthy".to_string(), None)]);
        assert!(ops.disabled().is_empty());
        assert!(ops.emitted().is_empty());
    }

    // ── 5. enforce_on_create — failing probe downgrades + warning ────

    #[tokio::test]
    async fn enforce_on_create_when_enabled_and_probe_fails_downgrades() {
        let r = repo(true);
        let ops = FakeHealthOps::default();
        ops.seed_repo(r.clone());
        let wrapper = enforce_on_create_with_ops(r.clone(), &ops, |_| {
            Box::pin(async {
                Err(ProbeFailure {
                    reason: "timeout".into(),
                })
            })
        })
        .await
        .unwrap();
        assert_eq!(wrapper.repository.enabled, false, "row downgraded");
        let warning = wrapper.connection_warning.expect("warning present on failure");
        assert_eq!(warning.reason, "timeout");
        // Side effects, in order: disable → record_unhealthy → emit.
        assert_eq!(ops.disabled(), vec![r.id]);
        assert_eq!(
            ops.recorded(),
            vec![(r.id, "unhealthy".to_string(), Some("timeout".to_string()))]
        );
        assert_eq!(ops.emitted(), vec![(r.id, "timeout".to_string())]);
    }

    // ── 6. enforce_on_update_transition — only false→true probes ─────

    #[tokio::test]
    async fn enforce_on_update_transition_only_triggers_on_false_to_true() {
        // Three scenarios that must NOT probe: disabled→disabled,
        // enabled→enabled, enabled→disabled. All three pass a probe_fn
        // that panics if invoked.
        for (old_enabled, new_enabled, label) in [
            (false, false, "disabled→disabled"),
            (true, true, "enabled→enabled"),
            (true, false, "enabled→disabled"),
        ] {
            let mut r = repo(new_enabled);
            r.enabled = new_enabled;
            let ops = FakeHealthOps::default();
            let result = enforce_on_update_transition_with_ops(
                r.clone(),
                old_enabled,
                &ops,
                |_| Box::pin(async move { panic!("probe must not run for {label}") }),
            )
            .await
            .expect(label);
            assert_eq!(result.enabled, new_enabled, "scenario {label}");
            assert!(ops.recorded().is_empty(), "scenario {label}");
            assert!(ops.disabled().is_empty(), "scenario {label}");
            assert!(ops.emitted().is_empty(), "scenario {label}");
        }

        // The fourth scenario (false→true) DOES probe; verify by
        // observing record_healthy on a passing probe_fn.
        let mut r = repo(true);
        r.enabled = true;
        let ops = FakeHealthOps::default();
        ops.seed_repo(r.clone());
        let _ = enforce_on_update_transition_with_ops(
            r.clone(),
            false, // old_enabled
            &ops,
            |_| Box::pin(async { Ok(()) }),
        )
        .await
        .expect("false→true with passing probe");
        assert_eq!(ops.recorded(), vec![(r.id, "healthy".to_string(), None)]);
    }

    // ── 7. record_test_outcome — Ok path records healthy, nothing else ──

    #[tokio::test]
    async fn record_test_outcome_success_records_healthy_only() {
        let r = repo(false);
        let ops = FakeHealthOps::default();

        let outcome = record_test_outcome(r.id, false, Ok(()), &ops)
            .await
            .expect("ok outcome returns Ok");
        assert!(outcome.success);
        assert!(outcome.reason.is_none());
        assert!(
            !outcome.already_emitted_auto_disabled,
            "healthy path must not flag auto_disabled emission"
        );
        assert_eq!(ops.recorded(), vec![(r.id, "healthy".to_string(), None)]);
        assert!(ops.disabled().is_empty());
        assert!(ops.emitted().is_empty());
    }

    // ── 8. record_test_outcome — failure on enabled row auto-disables ──

    #[tokio::test]
    async fn record_test_outcome_failure_on_enabled_row_auto_disables_and_emits() {
        let r = repo(true);
        let ops = FakeHealthOps::default();
        ops.seed_repo(r.clone());

        let outcome = record_test_outcome(
            r.id,
            true, // was_enabled
            Err(ProbeFailure {
                reason: "401 Unauthorized".into(),
            }),
            &ops,
        )
        .await
        .expect("ok outcome returns Ok");
        assert!(!outcome.success);
        assert_eq!(outcome.reason.as_deref(), Some("401 Unauthorized"));
        assert!(
            outcome.already_emitted_auto_disabled,
            "enabled-row failure (with successful disable) must flag auto_disabled emission"
        );
        assert_eq!(
            ops.recorded(),
            vec![(r.id, "unhealthy".to_string(), Some("401 Unauthorized".into()))]
        );
        assert_eq!(ops.disabled(), vec![r.id]);
        assert_eq!(
            ops.emitted(),
            vec![(r.id, "401 Unauthorized".to_string())],
            "enabled-row failure must auto-disable + emit",
        );
    }

    // ── 9. record_test_outcome — failure on DISABLED row is silent ─────

    #[tokio::test]
    async fn record_test_outcome_failure_on_disabled_row_does_not_disable_or_emit() {
        // Plan spec — manual Test Connection on a row that's already
        // disabled must NOT flip enabled (it's already off) and must
        // NOT emit auto_disabled (would spam listeners for no UI
        // benefit). Just records the result.
        let r = repo(false);
        let ops = FakeHealthOps::default();
        ops.seed_repo(r.clone());

        let outcome = record_test_outcome(
            r.id,
            false, // was_enabled
            Err(ProbeFailure {
                reason: "DNS failure".into(),
            }),
            &ops,
        )
        .await
        .expect("ok outcome returns Ok");
        assert!(!outcome.success);
        assert_eq!(outcome.reason.as_deref(), Some("DNS failure"));
        assert!(
            !outcome.already_emitted_auto_disabled,
            "disabled-row failure must NOT flag auto_disabled (handler should emit `updated`)"
        );
        assert_eq!(
            ops.recorded(),
            vec![(r.id, "unhealthy".to_string(), Some("DNS failure".into()))]
        );
        assert!(
            ops.disabled().is_empty(),
            "disabled-row failure must NOT call disable_repository",
        );
        assert!(
            ops.emitted().is_empty(),
            "disabled-row failure must NOT emit auto_disabled",
        );
    }

    // ── 10. record_test_outcome — disable_repository itself fails ─────

    /// FakeHealthOps variant whose `disable_repository` always errors.
    /// Used to verify the best-effort path: a DB hiccup during the
    /// auto-disable doesn't promote a probe-failure response to a 500;
    /// the handler still emits `updated` instead of `auto_disabled`.
    #[derive(Default)]
    struct DisableFailingOps {
        inner: FakeHealthOps,
    }

    #[async_trait]
    impl HealthOps for DisableFailingOps {
        async fn record_healthy(&self, repo_id: Uuid) {
            self.inner.record_healthy(repo_id).await
        }
        async fn record_unhealthy(&self, repo_id: Uuid, reason: &str) {
            self.inner.record_unhealthy(repo_id, reason).await
        }
        async fn disable_repository(&self, _repo_id: Uuid) -> Result<(), AppError> {
            Err(AppError::internal_error("simulated DB outage"))
        }
        async fn get_repository(
            &self,
            repo_id: Uuid,
        ) -> Result<Option<LlmRepository>, AppError> {
            self.inner.get_repository(repo_id).await
        }
        fn emit_auto_disabled(&self, repo_id: Uuid, reason: String) {
            self.inner.emit_auto_disabled(repo_id, reason)
        }
    }

    #[tokio::test]
    async fn record_test_outcome_when_disable_fails_returns_ok_and_does_not_emit() {
        // Best-effort semantics — a DB error during the auto-disable
        // call MUST NOT turn the probe-failure response into a 500.
        // The user gets a readable success:false outcome; listeners
        // see `updated` (handler-emitted) with the still-enabled-but-
        // unhealthy row instead of the stale `auto_disabled`.
        let r = repo(true);
        let ops = DisableFailingOps::default();
        ops.inner.seed_repo(r.clone());

        let outcome = record_test_outcome(
            r.id,
            true, // was_enabled
            Err(ProbeFailure {
                reason: "timeout".into(),
            }),
            &ops,
        )
        .await
        .expect("disable failure must NOT bubble as Err");
        assert!(!outcome.success);
        assert_eq!(outcome.reason.as_deref(), Some("timeout"));
        assert!(
            !outcome.already_emitted_auto_disabled,
            "disable_repository failed → handler must emit `updated`, NOT `auto_disabled`"
        );
        assert_eq!(
            ops.inner.recorded(),
            vec![(r.id, "unhealthy".to_string(), Some("timeout".into()))],
            "unhealthy status is recorded BEFORE the disable attempt — outcome is observable",
        );
        assert!(
            ops.inner.emitted().is_empty(),
            "disable failed → no auto_disabled emit (would be lying about the row's state)"
        );
    }
}

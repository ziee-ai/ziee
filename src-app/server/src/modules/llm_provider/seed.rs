//! Declarative seed provider for the `llm_providers` section — the relational subtree:
//! a provider owns nested `models` (keyed `(provider_id, name)`) and `assign_groups`
//! (a cross-reference to groups by name, resolved at apply time). The built-in default
//! providers are baked (built_in = true) by the llm_provider migration and ADOPTED here
//! (never re-created); this provider CREATEs operator-declared providers, converges their
//! models + group assignments idempotently, and — under reconcile / `remove:` — deletes
//! only ledger-owned rows.
//!
//! Ported from the app-side `core/seed` reference onto the `ziee-seed` SDK engine: the seam
//! types now come from `ziee_seed::`; `llm_models` lookup-by-name is an inline query (this
//! tree's `LlmModelRepository` exposes no `get_id_by_provider_and_name`).

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use linkme::distributed_slice;
use serde::Deserialize;
use uuid::Uuid;

use crate::core::Repos;
use crate::modules::llm_model::models::{EngineType, FileFormat};
use crate::modules::llm_model::types::CreateLlmModelRequest;
use crate::modules::llm_provider::types::{CreateLlmProviderRequest, UpdateLlmProviderRequest};
use ziee_seed::{
    SeedCtx, SeedEntry, SeedError, SeedMode, SeedOutcome, SeedProvider, SeedSection, SEED_PROVIDERS,
};

const SECTION: &str = "llm_providers";
const MODELS_SECTION: &str = "llm_models";
const ASSIGN_SECTION: &str = "user_group_llm_providers";

#[distributed_slice(SEED_PROVIDERS)]
static LLM_PROVIDERS_SEED: SeedEntry = SeedEntry {
    section: SECTION,
    order: 30,
    factory: || Arc::new(LlmProvidersSeedProvider),
};

pub struct LlmProvidersSeedProvider;

#[derive(Debug, Deserialize)]
struct ProviderItem {
    name: String,
    provider_type: String,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    /// `${ENV_VAR}` placeholder (routed through create/update's at-rest encryption).
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    models: Vec<ModelItem>,
    /// Group names this provider is made available to.
    #[serde(default)]
    assign_groups: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ModelItem {
    name: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    engine_type: Option<String>,
    #[serde(default)]
    file_format: Option<String>,
}

fn model_key(provider: &str, model: &str) -> String {
    format!("{provider}/{model}")
}
fn assign_key(provider: &str, group: &str) -> String {
    format!("{provider}:{group}")
}

impl LlmProvidersSeedProvider {
    /// Resolve a provider's `(id, built_in)` by natural key `name` (llm_providers has no
    /// UNIQUE(name), so the first match wins — the built-in defaults have distinct names).
    /// `built_in` lets the caller refuse to adopt an admin-created (built_in = false) row
    /// that merely shares a declared seed name (which reconcile-delete would then wrongly
    /// wipe).
    async fn find_by_name(pool: &sqlx::PgPool, name: &str) -> Result<Option<(Uuid, bool)>, SeedError> {
        let row = sqlx::query!(
            "SELECT id, built_in FROM llm_providers WHERE name = $1 ORDER BY created_at LIMIT 1",
            name
        )
        .fetch_optional(pool)
        .await?;
        Ok(row.map(|r| (r.id, r.built_in)))
    }

    /// This tree's `LlmModelRepository` has no by-name lookup; inline it.
    async fn model_id_by_name(pool: &sqlx::PgPool, provider_id: Uuid, name: &str) -> Result<Option<Uuid>, SeedError> {
        Ok(sqlx::query_scalar!(
            "SELECT id FROM llm_models WHERE provider_id = $1 AND name = $2 LIMIT 1",
            provider_id,
            name
        )
        .fetch_optional(pool)
        .await?)
    }

    async fn converge_models(
        &self,
        provider_id: Uuid,
        provider_name: &str,
        models: &[ModelItem],
        mode: SeedMode,
        ctx: &SeedCtx,
        outcome: &mut SeedOutcome,
    ) -> Result<(), SeedError> {
        let declared: HashSet<String> = models.iter().map(|m| m.name.clone()).collect();

        for m in models {
            let key = model_key(provider_name, &m.name);
            let existing = Self::model_id_by_name(&ctx.pool, provider_id, &m.name).await?;
            let seeded = ctx.ledger.is_seeded(MODELS_SECTION, &key).await?;
            match (existing, seeded) {
                (Some(id), false) => {
                    ctx.ledger.record(MODELS_SECTION, &key, Some(id)).await?;
                    outcome.adopted += 1;
                }
                (Some(_), true) => outcome.skipped += 1,
                (None, _) => {
                    let engine = m
                        .engine_type
                        .as_deref()
                        .and_then(EngineType::from_str)
                        .unwrap_or(EngineType::None);
                    let fmt = m
                        .file_format
                        .as_deref()
                        .and_then(FileFormat::from_str)
                        .unwrap_or(FileFormat::Safetensors);
                    let created = Repos.llm_model
                        .create(CreateLlmModelRequest {
                            provider_id,
                            name: m.name.clone(),
                            display_name: m.display_name.clone().unwrap_or_else(|| m.name.clone()),
                            description: None,
                            enabled: m.enabled,
                            capabilities: None,
                            parameters: None,
                            engine_type: engine,
                            engine_settings: None,
                            file_format: fmt,
                            required_runtime_version_id: None,
                        })
                        .await?;
                    ctx.ledger.record(MODELS_SECTION, &key, Some(created.id)).await?;
                    outcome.created += 1;
                }
            }
        }

        // Reconcile-delete: ledger-owned models under THIS provider absent from the list.
        if mode == SeedMode::Reconcile {
            let prefix = format!("{provider_name}/");
            for owned in ctx.ledger.list_owned(MODELS_SECTION).await? {
                if let Some(mname) = owned.natural_key.strip_prefix(&prefix) {
                    if !declared.contains(mname) {
                        if let Some(id) = owned.entity_id {
                            Repos.llm_model.delete(id).await?;
                        }
                        ctx.ledger.remove(MODELS_SECTION, &owned.natural_key).await?;
                        outcome.deleted += 1;
                    }
                }
            }
        }
        Ok(())
    }

    async fn converge_groups(
        &self,
        provider_id: Uuid,
        provider_name: &str,
        groups: &[String],
        mode: SeedMode,
        ctx: &SeedCtx,
        _outcome: &mut SeedOutcome,
    ) -> Result<(), SeedError> {
        for gname in groups {
            let Some(group) = Repos.group.get_by_name(gname).await? else {
                tracing::warn!(provider = %provider_name, group = %gname, "seed: group not found; provider not assigned");
                continue;
            };
            // Idempotent (ON CONFLICT DO NOTHING).
            Repos.user_group_llm_provider
                .assign_to_group(provider_id, group.id)
                .await?;
            ctx.ledger
                .record(ASSIGN_SECTION, &assign_key(provider_name, gname), Some(group.id))
                .await?;
        }

        // Reconcile-revoke: a seed-owned assignment for THIS provider no longer in the
        // declared group list is revoked (seed-if-empty stays additive). We enumerate THIS
        // provider's ACTUAL current assignments by provider_id and revoke only the ones that
        // are seed-owned (exact ledger key) and undeclared — an admin-made assignment (absent
        // from the ledger) is never touched.
        if mode == SeedMode::Reconcile {
            let declared: HashSet<&str> = groups.iter().map(String::as_str).collect();
            let current = sqlx::query!(
                "SELECT g.id, g.name FROM user_group_llm_providers ugp \
                 JOIN groups g ON g.id = ugp.group_id WHERE ugp.provider_id = $1",
                provider_id
            )
            .fetch_all(&ctx.pool)
            .await?;
            for row in current {
                if declared.contains(row.name.as_str()) {
                    continue;
                }
                let key = assign_key(provider_name, &row.name);
                if ctx.ledger.is_seeded(ASSIGN_SECTION, &key).await? {
                    Repos.user_group_llm_provider
                        .remove_from_group(row.id, provider_id)
                        .await?;
                    ctx.ledger.remove(ASSIGN_SECTION, &key).await?;
                }
            }
        }
        Ok(())
    }

    /// Delete a ledger-owned provider + its owned subtree (models/assignments cascade in
    /// the DB via FK ON DELETE CASCADE; the ledger rows are cleaned up here).
    async fn delete_owned(
        &self,
        name: &str,
        entity_id: Option<Uuid>,
        ctx: &SeedCtx,
        outcome: &mut SeedOutcome,
    ) -> Result<(), SeedError> {
        if let Some(id) = entity_id {
            let _ = Repos.llm_provider.delete(id).await?;
        }
        ctx.ledger.remove(SECTION, name).await?;
        for owned in ctx.ledger.list_owned(MODELS_SECTION).await? {
            if owned.natural_key.starts_with(&format!("{name}/")) {
                ctx.ledger.remove(MODELS_SECTION, &owned.natural_key).await?;
            }
        }
        for owned in ctx.ledger.list_owned(ASSIGN_SECTION).await? {
            if owned.natural_key.starts_with(&format!("{name}:")) {
                ctx.ledger.remove(ASSIGN_SECTION, &owned.natural_key).await?;
            }
        }
        outcome.deleted += 1;
        Ok(())
    }
}

#[async_trait]
impl SeedProvider for LlmProvidersSeedProvider {
    fn section(&self) -> &'static str {
        SECTION
    }

    async fn apply(
        &self,
        section: Option<&SeedSection>,
        mode: SeedMode,
        ctx: &SeedCtx,
    ) -> Result<SeedOutcome, SeedError> {
        let mut outcome = SeedOutcome::default();
        let Some(section) = section else {
            return Ok(outcome);
        };
        let pool = &ctx.pool;

        let mut declared_names: HashSet<String> = HashSet::new();

        for raw in &section.items {
            let item: ProviderItem = match serde_norway::from_value(raw.clone()) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(section = SECTION, error = %e, "seed: skipping malformed llm_providers item");
                    outcome.skipped += 1;
                    continue;
                }
            };
            declared_names.insert(item.name.clone());

            // A secret api_key that is unset/inline simply means "no key configured" — never
            // a reason to skip the whole provider (it can be keyed later in the UI).
            let resolved_key = match &item.api_key {
                Some(raw) => match ctx.resolve_secret(raw) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        tracing::debug!(provider = %item.name, reason = %e, "seed: provider api_key not set; creating without a key");
                        None
                    }
                },
                None => None,
            };

            let existing = Self::find_by_name(pool, &item.name).await?;
            let seeded = ctx.ledger.is_seeded(SECTION, &item.name).await?;

            // A once-seeded default the admin later DELETED must NOT be resurrected by
            // seed-if-empty (ledger-gated — only a wipe drops the ledger). Only reconcile
            // (authoritative desired-state) re-creates it.
            if existing.is_none() && seeded && mode != SeedMode::Reconcile {
                outcome.skipped += 1;
                continue;
            }

            let provider_id = match (existing, seeded) {
                // Owned already: reconcile re-syncs declared scalars; seed-if-empty leaves them.
                (Some((id, _)), true) => {
                    if mode == SeedMode::Reconcile {
                        Repos.llm_provider
                            .update(
                                id,
                                UpdateLlmProviderRequest {
                                    name: None,
                                    enabled: item.enabled,
                                    api_key: resolved_key.clone(),
                                    base_url: item.base_url.clone(),
                                    proxy_settings: None,
                                },
                            )
                            .await?;
                        outcome.updated += 1;
                    } else {
                        outcome.skipped += 1;
                    }
                    id
                }
                // Present but not yet owned → adopt in place ONLY if it is a genuine seed row
                // (built_in). An admin-created row (built_in = false) that merely shares the
                // name is neither adopted nor duplicated — otherwise reconcile-delete would
                // later wipe it.
                (Some((id, built_in)), false) => {
                    if !built_in {
                        tracing::warn!(provider = %item.name, "seed: an admin-created provider already has this name; not adopting or duplicating");
                        outcome.skipped += 1;
                        continue;
                    }
                    ctx.ledger.record(SECTION, &item.name, Some(id)).await?;
                    outcome.adopted += 1;
                    id
                }
                // Absent → create.
                (None, _) => {
                    let created = Repos.llm_provider
                        .create(CreateLlmProviderRequest {
                            name: item.name.clone(),
                            provider_type: item.provider_type.clone(),
                            enabled: item.enabled,
                            api_key: resolved_key.clone(),
                            base_url: item.base_url.clone(),
                            proxy_settings: None,
                        })
                        .await?;
                    ctx.ledger.record(SECTION, &item.name, Some(created.id)).await?;
                    outcome.created += 1;
                    created.id
                }
            };

            // Additive relations always converge (idempotent) — a provider assigned to no
            // group is unusable by non-admin users, so this must be self-healing.
            self.converge_models(provider_id, &item.name, &item.models, mode, ctx, &mut outcome)
                .await?;
            self.converge_groups(provider_id, &item.name, &item.assign_groups, mode, ctx, &mut outcome)
                .await?;
        }

        // Explicit `remove:` — delete these specific ledger-owned providers (both modes).
        for name in &section.remove {
            if let Some(row) = ctx.ledger.lookup(SECTION, name).await? {
                self.delete_owned(name, row.entity_id, ctx, &mut outcome).await?;
            }
        }

        // Reconcile-delete: ledger-owned providers absent from the declared list.
        if mode == SeedMode::Reconcile {
            for owned in ctx.ledger.list_owned(SECTION).await? {
                if !declared_names.contains(&owned.natural_key) {
                    self.delete_owned(&owned.natural_key, owned.entity_id, ctx, &mut outcome)
                        .await?;
                }
            }
        }

        Ok(outcome)
    }

    async fn dump(&self, ctx: &SeedCtx) -> Result<Option<serde_norway::Value>, SeedError> {
        let owned = ctx.ledger.list_owned(SECTION).await?;
        if owned.is_empty() {
            return Ok(None);
        }
        let mut items = Vec::new();
        for row in owned {
            let Some(id) = row.entity_id else { continue };
            let Some(p) = Repos.llm_provider.get_by_id(id).await? else { continue };
            let has_key = sqlx::query_scalar!(
                "SELECT (api_key IS NOT NULL OR api_key_encrypted IS NOT NULL) FROM llm_providers WHERE id = $1",
                id
            )
            .fetch_one(&ctx.pool)
            .await?
            .unwrap_or(false);
            let models = Repos.llm_model.list_by_provider(id).await?;
            let model_names: Vec<(String, String)> =
                models.into_iter().map(|m| (m.name, m.display_name)).collect();
            let groups: Vec<String> = sqlx::query_scalar!(
                "SELECT g.name FROM user_group_llm_providers ugp JOIN groups g ON g.id = ugp.group_id WHERE ugp.provider_id = $1 ORDER BY g.name",
                id
            )
            .fetch_all(&ctx.pool)
            .await?;
            items.push(provider_dump_value(
                &p.name,
                &p.provider_type,
                p.base_url.as_deref(),
                p.enabled,
                has_key,
                &model_names,
                &groups,
            ));
        }
        Ok(Some(serde_norway::to_value(serde_json::json!({ "items": items }))
            .map_err(|e| SeedError::Other(e.to_string()))?))
    }
}

/// Pure builder for a provider's dump YAML value — nests `models` (keyed by name) and
/// `assign_groups`, and renders a configured api_key as a `${…}` placeholder (never the
/// value). Factored out so the dump SHAPE is unit-testable without a database.
pub fn provider_dump_value(
    name: &str,
    provider_type: &str,
    base_url: Option<&str>,
    enabled: bool,
    has_api_key: bool,
    models: &[(String, String)],
    groups: &[String],
) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    obj.insert("name".into(), name.into());
    obj.insert("provider_type".into(), provider_type.into());
    if let Some(b) = base_url {
        obj.insert("base_url".into(), b.into());
    }
    obj.insert("enabled".into(), enabled.into());
    if has_api_key {
        obj.insert(
            "api_key".into(),
            format!("${{{}_API_KEY}}", name.to_uppercase().replace([' ', '-'], "_")).into(),
        );
    }
    if !models.is_empty() {
        let ms: Vec<serde_json::Value> = models
            .iter()
            .map(|(n, d)| serde_json::json!({ "name": n, "display_name": d }))
            .collect();
        obj.insert("models".into(), serde_json::Value::Array(ms));
    }
    if !groups.is_empty() {
        obj.insert(
            "assign_groups".into(),
            serde_json::Value::Array(groups.iter().map(|g| g.clone().into()).collect()),
        );
    }
    serde_json::Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dump_shape_nests_models_and_groups_and_placeholders_the_secret() {
        let v = provider_dump_value(
            "OpenAI",
            "openai",
            Some("https://api.openai.com/v1"),
            true,
            true,
            &[("gpt-4o".to_string(), "GPT-4o".to_string())],
            &["Users".to_string()],
        );
        assert_eq!(v["name"], "OpenAI");
        assert_eq!(v["base_url"], "https://api.openai.com/v1");
        assert_eq!(v["enabled"], true);
        assert_eq!(v["api_key"], "${OPENAI_API_KEY}");
        assert_eq!(v["models"][0]["name"], "gpt-4o");
        assert_eq!(v["models"][0]["display_name"], "GPT-4o");
        assert_eq!(v["assign_groups"][0], "Users");

        let v2 = provider_dump_value("Local", "local", None, false, false, &[], &[]);
        assert!(v2.get("api_key").is_none());
        assert!(v2.get("models").is_none());
        assert!(v2.get("assign_groups").is_none());
    }
}

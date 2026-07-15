//! Declarative seed provider for the `assistants` section (system templates only:
//! `is_template = true`, `created_by = NULL`). The Default Assistant is baked by the
//! assistant migration and ADOPTED here; operator-declared templates are created
//! idempotently by name. Ported onto the `ziee-seed` SDK engine.

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use linkme::distributed_slice;
use serde::Deserialize;
use uuid::Uuid;

use crate::core::Repos;
use crate::modules::assistant::repository::create_assistant;
use crate::modules::assistant::types::CreateAssistantRequest;
use ziee_seed::{
    SeedCtx, SeedEntry, SeedError, SeedMode, SeedOutcome, SeedProvider, SeedSection, SEED_PROVIDERS,
};

const SECTION: &str = "assistants";

#[distributed_slice(SEED_PROVIDERS)]
static ASSISTANTS_SEED: SeedEntry = SeedEntry {
    section: SECTION,
    order: 45,
    factory: || Arc::new(AssistantsSeedProvider),
};

pub struct AssistantsSeedProvider;

#[derive(Debug, Deserialize)]
struct AssistantItem {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    instructions: Option<String>,
    #[serde(default)]
    is_default: Option<bool>,
}

async fn find_template(pool: &sqlx::PgPool, name: &str) -> Result<Option<Uuid>, SeedError> {
    Ok(sqlx::query_scalar!(
        "SELECT id FROM assistants WHERE name = $1 AND is_template = true LIMIT 1",
        name
    )
    .fetch_optional(pool)
    .await?)
}

#[async_trait]
impl SeedProvider for AssistantsSeedProvider {
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
        let Some(section) = section else { return Ok(outcome) };
        let pool = &ctx.pool;
        let mut declared: HashSet<String> = HashSet::new();

        for raw in &section.items {
            let item: AssistantItem = match serde_norway::from_value(raw.clone()) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(section = SECTION, error = %e, "seed: skipping malformed assistants item");
                    outcome.skipped += 1;
                    continue;
                }
            };
            declared.insert(item.name.clone());

            let existing = find_template(pool, &item.name).await?;
            let seeded = ctx.ledger.is_seeded(SECTION, &item.name).await?;

            // A once-seeded default the admin later DELETED must NOT be resurrected by
            // seed-if-empty (ledger-gated); only reconcile re-creates it.
            if existing.is_none() && seeded && mode != SeedMode::Reconcile {
                outcome.skipped += 1;
                continue;
            }

            match (existing, seeded) {
                (Some(id), false) => {
                    ctx.ledger.record(SECTION, &item.name, Some(id)).await?;
                    outcome.adopted += 1;
                }
                (Some(_), true) => outcome.skipped += 1,
                (None, _) => {
                    let created = create_assistant(
                        pool,
                        None,
                        CreateAssistantRequest {
                            name: item.name.clone(),
                            description: item.description.clone(),
                            instructions: item.instructions.clone(),
                            parameters: None,
                            is_template: Some(true),
                            is_default: item.is_default,
                            enabled: Some(true),
                        },
                    )
                    .await?;
                    ctx.ledger.record(SECTION, &item.name, Some(created.id)).await?;
                    outcome.created += 1;
                }
            }
        }

        // Explicit `remove:` — delete eagerly and clear the ledger row FIRST, so a name that
        // is both removed AND absent from `declared` can't be seen (and deleted) again by the
        // reconcile-missing pass below (mirrors llm_provider).
        for name in &section.remove {
            if let Some(row) = ctx.ledger.lookup(SECTION, name).await? {
                if let Some(id) = row.entity_id {
                    Repos.assistant.delete(id).await?;
                }
                ctx.ledger.remove(SECTION, name).await?;
                outcome.deleted += 1;
            }
        }

        // Reconcile-delete: ledger-owned templates absent from the declared list.
        if mode == SeedMode::Reconcile {
            for owned in ctx.ledger.list_owned(SECTION).await? {
                if !declared.contains(&owned.natural_key) {
                    if let Some(id) = owned.entity_id {
                        Repos.assistant.delete(id).await?;
                    }
                    ctx.ledger.remove(SECTION, &owned.natural_key).await?;
                    outcome.deleted += 1;
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
            let r = sqlx::query!(
                "SELECT name, description, instructions, is_default FROM assistants WHERE name = $1 AND is_template = true LIMIT 1",
                row.natural_key
            )
            .fetch_optional(&ctx.pool)
            .await?;
            if let Some(r) = r {
                items.push(serde_norway::to_value(serde_json::json!({
                    "name": r.name,
                    "description": r.description,
                    "instructions": r.instructions,
                    "is_default": r.is_default,
                })).map_err(|e| SeedError::Other(e.to_string()))?);
            }
        }
        Ok(Some(serde_norway::to_value(serde_json::json!({ "items": items }))
            .map_err(|e| SeedError::Other(e.to_string()))?))
    }
}

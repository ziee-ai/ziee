//! Durable Postgres impl of the agent-core [`TaskListStore`] port (Group G /
//! ITEM-34/35, DEC-49/50) — the server half of the agent self-task-management
//! feature. The crate defines the port (DB-free); this is the concrete
//! `agent_task_list`-backed store, mirroring how `WorkflowTranscriptStore`
//! supplies the durable `TranscriptStore` port inline (a port impl holding a
//! `PgPool`, not a global `Repos` entry).
//!
//! **Keyed purely by `run_id`** — chat keys by the assistant message id, the
//! workflow-agent step by `workflow_runs.id`, and each fan-out child gets a
//! FRESH `run_id`, so every agent / sub-agent has its OWN run-scoped list and
//! the parent never reads a child's (ITEM-37 / DEC-53 — structural isolation,
//! no rollup). The store is the SOURCE OF TRUTH for the list (DEC-52): the
//! re-injection extension re-renders from here, not the transcript, which is
//! what makes "survive compaction" trivially true.
//!
//! Runtime `sqlx::query_as` (no compile-time `query!` macros) — same convention
//! as `agent::repository`.

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use agent_core::{TaskItem, TaskItemCreate, TaskItemPatch, TaskListStore, TaskStatus};

use crate::common::AppError;

/// One `agent_task_list` row (runtime `FromRow` — mapped by column name).
#[derive(sqlx::FromRow)]
struct TaskListRow {
    id: Uuid,
    content: String,
    active_form: String,
    status: String,
    owner: Option<String>,
    deps: serde_json::Value,
}

impl TaskListRow {
    fn into_item(self) -> TaskItem {
        TaskItem {
            id: self.id,
            content: self.content,
            active_form: self.active_form,
            status: status_from_str(&self.status),
            owner: self.owner,
            deps: deps_from_json(&self.deps),
        }
    }
}

/// `TaskStatus` → the DB CHECK vocabulary (`pending` / `in_progress` / `completed`).
fn status_to_str(s: TaskStatus) -> &'static str {
    match s {
        TaskStatus::Pending => "pending",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::Completed => "completed",
    }
}

/// DB text → `TaskStatus`; an unrecognized value degrades to `Pending` rather
/// than panicking (§6 — never `unwrap()` on an enum string from the DB).
fn status_from_str(s: &str) -> TaskStatus {
    match s {
        "in_progress" => TaskStatus::InProgress,
        "completed" => TaskStatus::Completed,
        _ => TaskStatus::Pending,
    }
}

/// `deps: &[Uuid]` → a jsonb array of stringified uuids.
fn deps_to_json(deps: &[Uuid]) -> serde_json::Value {
    serde_json::Value::Array(
        deps.iter()
            .map(|u| serde_json::Value::String(u.to_string()))
            .collect(),
    )
}

/// jsonb array of stringified uuids → `Vec<Uuid>` (unparseable entries dropped).
fn deps_from_json(v: &serde_json::Value) -> Vec<Uuid> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .filter_map(|s| Uuid::parse_str(s).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Durable per-run agent task list on the `agent_task_list` table.
pub struct PgTaskListStore {
    pool: PgPool,
}

impl PgTaskListStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TaskListStore for PgTaskListStore {
    async fn load(&self, run_id: Uuid) -> Result<Vec<TaskItem>, AppError> {
        let rows: Vec<TaskListRow> = sqlx::query_as(
            r#"
            SELECT id, content, active_form, status, owner, deps
            FROM agent_task_list
            WHERE run_id = $1
            ORDER BY position ASC, created_at ASC
            "#,
        )
        .bind(run_id)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(rows.into_iter().map(TaskListRow::into_item).collect())
    }

    async fn create(&self, run_id: Uuid, item: TaskItemCreate) -> Result<TaskItem, AppError> {
        let status = status_to_str(item.status.unwrap_or(TaskStatus::Pending));
        let deps = deps_to_json(&item.deps);
        // `position` = next slot for this run (append-at-end ordering) so `load`
        // returns items in creation order.
        let row: TaskListRow = sqlx::query_as(
            r#"
            INSERT INTO agent_task_list
                (run_id, content, active_form, status, owner, deps, position)
            VALUES (
                $1, $2, $3, $4, $5, $6,
                COALESCE((SELECT MAX(position) + 1 FROM agent_task_list WHERE run_id = $1), 0)
            )
            RETURNING id, content, active_form, status, owner, deps
            "#,
        )
        .bind(run_id)
        .bind(&item.content)
        .bind(&item.active_form)
        .bind(status)
        .bind(item.owner.as_deref())
        .bind(deps)
        .fetch_one(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.into_item())
    }

    async fn update(
        &self,
        run_id: Uuid,
        item_id: Uuid,
        patch: TaskItemPatch,
    ) -> Result<TaskItem, AppError> {
        // Partial patch: COALESCE each supplied field. `deps: Some(vec![])`
        // clears; `deps: None` leaves it untouched (bool-guarded CASE). `owner`
        // is a plain `Option<String>` (present ⇒ set, absent ⇒ leave — the patch
        // shape has no "clear owner to null" affordance).
        let status = patch.status.map(status_to_str);
        let owner_set = patch.owner.is_some();
        let deps_set = patch.deps.is_some();
        let deps_val = patch.deps.as_ref().map(|d| deps_to_json(d));
        let row: Option<TaskListRow> = sqlx::query_as(
            r#"
            UPDATE agent_task_list SET
                content     = COALESCE($3, content),
                active_form = COALESCE($4, active_form),
                status      = COALESCE($5, status),
                owner       = CASE WHEN $6::bool THEN $7 ELSE owner END,
                deps        = CASE WHEN $8::bool THEN $9 ELSE deps END,
                updated_at  = NOW()
            WHERE run_id = $1 AND id = $2
            RETURNING id, content, active_form, status, owner, deps
            "#,
        )
        .bind(run_id)
        .bind(item_id)
        .bind(patch.content.as_deref())
        .bind(patch.active_form.as_deref())
        .bind(status)
        .bind(owner_set)
        .bind(patch.owner.as_deref())
        .bind(deps_set)
        .bind(deps_val)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        // A missing id is an error the loop surfaces to the model as an `is_error`
        // tool result (the port contract).
        row.map(TaskListRow::into_item).ok_or_else(|| {
            AppError::bad_request(
                "TASK_ITEM_NOT_FOUND",
                format!("no task item {item_id} in this run"),
            )
        })
    }

    async fn get(&self, run_id: Uuid, item_id: Uuid) -> Result<Option<TaskItem>, AppError> {
        let row: Option<TaskListRow> = sqlx::query_as(
            r#"
            SELECT id, content, active_form, status, owner, deps
            FROM agent_task_list
            WHERE run_id = $1 AND id = $2
            "#,
        )
        .bind(run_id)
        .bind(item_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::database_error)?;
        Ok(row.map(TaskListRow::into_item))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_roundtrips_through_db_vocabulary() {
        for s in [TaskStatus::Pending, TaskStatus::InProgress, TaskStatus::Completed] {
            assert_eq!(status_from_str(status_to_str(s)), s);
        }
        // Unknown DB text degrades to Pending (never panics).
        assert_eq!(status_from_str("garbage"), TaskStatus::Pending);
    }

    #[test]
    fn deps_json_roundtrips() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let json = deps_to_json(&[a, b]);
        assert_eq!(deps_from_json(&json), vec![a, b]);
        // Empty + non-array degrade to an empty vec.
        assert!(deps_from_json(&serde_json::json!([])).is_empty());
        assert!(deps_from_json(&serde_json::json!("nope")).is_empty());
    }
}

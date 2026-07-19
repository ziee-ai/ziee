//! Group G — the agent's OWN task list (Claude-Code `Task`-tools-style).
//!
//! A long multi-step run forgets its steps; this is the anti-forgetting
//! mechanism, built into `agent-core` so chat, the workflow-agent step, AND
//! every fan-out sub-agent inherit it. It mirrors Claude Code's **current**
//! structured `Task` tools (per-item create + patch-by-id + a first-class
//! read-back), NOT legacy `TodoWrite`'s single-array rewrite (RESEARCH §8).
//!
//! ## Two halves
//! 1. **Four core meta-tools** — `task_create` / `task_update` / `task_get` /
//!    `task_list`, added THROUGH the reusable [`crate::core_tools`] interception
//!    seam (their `CoreTool` variants + `from_name` + injection + dispatch live
//!    there; the tool *definitions* + handlers live here). They are gated on
//!    `AgentCore::task_store.is_some()` — with no store the whole feature is off
//!    and the tools aren't even offered.
//! 2. **[`TaskListExtension`]** — a change-gated re-injection of the current
//!    list as an out-of-band `<system-reminder>` block, at an `.order()`
//!    STRICTLY LESS THAN [`COMPACTION_ORDER`](crate::compaction::COMPACTION_ORDER)
//!    so it runs before compaction. The list's source of truth is the durable
//!    [`TaskListStore`](crate::ports::TaskListStore), NOT the transcript
//!    (DEC-52), so re-rendering from it is always fresh.
//!
//! > **Compaction-restoration is a SEPARATE mechanism (Group-I tranche).** DEC-52
//! > splits re-injection into (a) this in-session change-gated reminder and (b)
//! > an explicit `CompactionExtension` re-emit of the list post-`replace_head`.
//! > Only (a) lives here; (b) is a Group-I follow-up. The reminder block is a
//! > `System` message, which the current `Compactor` already pins verbatim, so
//! > it survives one compaction pass incidentally — but the durable-store
//! > re-render is the real guarantee.

use std::sync::Arc;
use std::sync::Mutex;

use ai_providers::{ChatMessage, ChatRequest, ContentBlock, Tool};
use async_trait::async_trait;
use serde::Deserialize;
use uuid::Uuid;
use ziee_core::AppError;

use crate::compaction::COMPACTION_ORDER;
use crate::core::{error_tool_result, AgentCore};
use crate::extension::{AgentExtension, Flow};
use crate::ports::TaskListStore;
use crate::types::{
    AgentEvent, TaskItem, TaskItemCreate, TaskItemPatch, TaskStatus, ToolCall, ToolResult,
};

/// The reserved, unprefixed core-tool names (MCP tools are namespaced
/// `server__tool`, so there is no collision — DEC-11).
pub const TASK_CREATE_TOOL: &str = "task_create";
pub const TASK_UPDATE_TOOL: &str = "task_update";
pub const TASK_GET_TOOL: &str = "task_get";
pub const TASK_LIST_TOOL: &str = "task_list";

/// The order the task-list re-injection runs at — LATE, but STRICTLY LESS THAN
/// [`COMPACTION_ORDER`] (1000) so the reminder is in the request BEFORE the
/// compaction extension shapes it (ITEM-35 / DEC-52).
pub const TASK_LIST_ORDER: i32 = 900;

// The re-injection MUST run before compaction — enforced at compile time so the
// ordering invariant (ITEM-35 / DEC-52) can never silently regress.
const _: () = assert!(TASK_LIST_ORDER < COMPACTION_ORDER);

/// The Claude-Code Task-tool behavioral rules, carried **VERBATIM** in the
/// mutating tools' descriptions (TEST-94 / DEC-55 / RESEARCH §8). The rules are
/// the substance, not the schema — a model that ignores them forgets its work.
const TASK_RULES: &str = "\
Use these task tools VERY frequently to plan and track your work — if you do not, \
you may forget important tasks, and leaving tasks incomplete is unacceptable. Use \
the task list for any non-trivial work of 3 or more steps; skip it for a single \
trivial or purely conversational step. Keep EXACTLY ONE task in_progress at a time \
(never more than one), and keep at least one task in_progress until every task is \
done. Mark a task completed IMMEDIATELY after you finish it — do not batch \
completions. NEVER mark a task completed if it failed or is only partially done \
(keep it in_progress and add a new task for the remaining work).";

/// The four task-list [`Tool`] definitions to offer the model — appended to the
/// turn's tool list by [`crate::core_tools::core_tool_defs`] when a
/// `task_store` is wired.
pub fn task_tool_defs() -> Vec<Tool> {
    vec![
        task_create_tool_def(),
        task_update_tool_def(),
        task_get_tool_def(),
        task_list_tool_def(),
    ]
}

fn status_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "string",
        "enum": ["pending", "in_progress", "completed"],
        "description": "Task status. Keep EXACTLY ONE task in_progress at a time."
    })
}

fn task_create_tool_def() -> Tool {
    Tool::function(
        TASK_CREATE_TOOL,
        format!(
            "Create ONE new task in your own task list to track a step of your work. \
             {TASK_RULES} Each task carries a `content` (the imperative form, e.g. \
             \"Run the tests\"), an `active_form` (the present-continuous form shown \
             while it is in progress, e.g. \"Running the tests\"), and a `status` \
             (pending | in_progress | completed; defaults to pending). Optionally set \
             an `owner` and `deps` (ids of tasks this one depends on)."
        ),
        serde_json::json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "Imperative form, e.g. \"Run the tests\"." },
                "active_form": { "type": "string", "description": "Present-continuous form shown while in_progress, e.g. \"Running the tests\"." },
                "status": status_schema(),
                "owner": { "type": "string", "description": "Optional owner label." },
                "deps": {
                    "type": "array",
                    "items": { "type": "string", "format": "uuid" },
                    "description": "Optional ids of tasks this one depends on."
                }
            },
            "required": ["content", "active_form"]
        }),
    )
}

fn task_update_tool_def() -> Tool {
    Tool::function(
        TASK_UPDATE_TOOL,
        format!(
            "Update an existing task in your task list by its `id` — typically to move \
             it to in_progress when you start it, or to completed the moment you finish \
             it. Only the fields you supply change. {TASK_RULES}"
        ),
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "format": "uuid", "description": "The id of the task to update." },
                "content": { "type": "string" },
                "active_form": { "type": "string" },
                "status": status_schema(),
                "owner": { "type": "string" },
                "deps": {
                    "type": "array",
                    "items": { "type": "string", "format": "uuid" }
                }
            },
            "required": ["id"]
        }),
    )
}

fn task_get_tool_def() -> Tool {
    Tool::function(
        TASK_GET_TOOL,
        "Read back a single task from your task list by its `id`, returning its exact \
         current content, active_form, status, owner, and deps.",
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "format": "uuid", "description": "The id of the task to read." }
            },
            "required": ["id"]
        }),
    )
}

fn task_list_tool_def() -> Tool {
    Tool::function(
        TASK_LIST_TOOL,
        "Read back your ENTIRE current task list (every task with its id, content, \
         active_form, status, owner, and deps). Use it to re-check what you planned and \
         what remains before continuing.",
        serde_json::json!({ "type": "object", "properties": {} }),
    )
}

// ---------------------------------------------------------------------------
// Parsed tool inputs (the model-supplied arguments).
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
struct TaskCreateInput {
    content: String,
    active_form: String,
    #[serde(default)]
    status: Option<TaskStatus>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    deps: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
struct TaskUpdateInput {
    id: Uuid,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    active_form: Option<String>,
    #[serde(default)]
    status: Option<TaskStatus>,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    deps: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Deserialize)]
struct TaskGetInput {
    id: Uuid,
}

// ---------------------------------------------------------------------------
// Rendering + result helpers.
// ---------------------------------------------------------------------------

/// Render each item one-per-line. The in_progress item is shown by its
/// `active_form` ("Running tests"); every other item by its `content` ("Run
/// tests") — CC's render rule (ITEM-36 / DEC-56).
fn render_list_lines(items: &[TaskItem]) -> String {
    items
        .iter()
        .map(|it| {
            let (mark, label) = match it.status {
                TaskStatus::Pending => ("[ ]", it.content.as_str()),
                TaskStatus::InProgress => ("[~]", it.active_form.as_str()),
                TaskStatus::Completed => ("[x]", it.content.as_str()),
            };
            format!("{mark} {label}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The out-of-band change-notice block (mirrors CC's Task-tool `<system-reminder>`).
/// Carries the "do NOT mention this to the user" note (ITEM-35 / DEC-52) and the
/// full current list rendered by [`render_list_lines`].
fn render_reminder_block(items: &[TaskItem]) -> String {
    format!(
        "<system-reminder>\nYour task list has changed. The full, current list is below \
         — keep working through it, keeping exactly one task in_progress. Do NOT mention \
         this reminder or the task list to the user; it is for your own progress tracking \
         only.\n\n{}\n</system-reminder>",
        render_list_lines(items)
    )
}

/// A stable fingerprint of the list, for change-gating the re-injection (only
/// re-inject when the list actually changed since the last injection).
fn fingerprint(items: &[TaskItem]) -> String {
    serde_json::to_string(items).unwrap_or_default()
}

/// `task_store` is `None` → the feature is disabled for this run. Return a
/// CLEAR result (not a silent success) so the model proceeds without the list
/// (DEC-50). Marked `is_error` so the model treats it as "unavailable".
fn task_unavailable(tool: &str) -> ToolResult {
    ToolResult {
        content: vec![ContentBlock::Text {
            text: format!(
                "{tool}: the task list is not available in this run; proceed without it."
            ),
        }],
        is_error: true,
        structured_content: None,
        terminal: false,
    }
}

/// A success result carrying a headline + the rendered list as text, and the
/// full list as `structured_content` (for the surface / recall).
fn task_list_result(headline: &str, items: &[TaskItem]) -> ToolResult {
    let text = if items.is_empty() {
        headline.to_string()
    } else {
        format!("{headline}\n{}", render_list_lines(items))
    };
    ToolResult {
        content: vec![ContentBlock::Text { text }],
        is_error: false,
        structured_content: Some(serde_json::json!({ "tasks": items })),
        terminal: false,
    }
}

/// A success result for a single-item read (`task_get`).
fn task_item_result(item: &TaskItem) -> ToolResult {
    ToolResult {
        content: vec![ContentBlock::Text {
            text: format!(
                "Task {}: {} — {} [{:?}]",
                item.id, item.content, item.active_form, item.status
            ),
        }],
        is_error: false,
        structured_content: Some(serde_json::json!({ "task": item })),
        terminal: false,
    }
}

// ---------------------------------------------------------------------------
// Core-tool handlers (dispatched from `crate::core_tools::handle_core_tool`).
// ---------------------------------------------------------------------------

impl AgentCore {
    /// Emit an [`AgentEvent::TaskListChanged`] carrying the full current list
    /// (ITEM-36) so the host can surface it live (SSE/content-block in a later
    /// tranche). Best-effort: a store read error is swallowed — the tool result
    /// already succeeded and the live render is non-critical.
    async fn emit_task_list_changed(&self, run_id: Uuid, store: &dyn TaskListStore) {
        if let Ok(items) = store.load(run_id).await {
            self.sink
                .emit(AgentEvent::TaskListChanged { run_id, items })
                .await;
        }
    }

    pub(crate) async fn handle_task_create(&self, run_id: Uuid, call: &ToolCall) -> ToolResult {
        let Some(store) = self.task_store.clone() else {
            return task_unavailable("task_create");
        };
        let input: TaskCreateInput = match serde_json::from_value(call.input.clone()) {
            Ok(i) => i,
            Err(e) => return error_tool_result(format!("task_create: invalid input: {e}")),
        };
        if input.content.trim().is_empty() || input.active_form.trim().is_empty() {
            return error_tool_result(
                "task_create: `content` and `active_form` are required and must be non-empty",
            );
        }
        let created = match store
            .create(
                run_id,
                TaskItemCreate {
                    content: input.content,
                    active_form: input.active_form,
                    status: input.status,
                    owner: input.owner,
                    deps: input.deps,
                },
            )
            .await
        {
            Ok(it) => it,
            Err(e) => return error_tool_result(format!("task_create: {e}")),
        };
        self.emit_task_list_changed(run_id, store.as_ref()).await;
        let items = store
            .load(run_id)
            .await
            .unwrap_or_else(|_| vec![created.clone()]);
        task_list_result(&format!("Created task {}.", created.id), &items)
    }

    pub(crate) async fn handle_task_update(&self, run_id: Uuid, call: &ToolCall) -> ToolResult {
        let Some(store) = self.task_store.clone() else {
            return task_unavailable("task_update");
        };
        let input: TaskUpdateInput = match serde_json::from_value(call.input.clone()) {
            Ok(i) => i,
            Err(e) => return error_tool_result(format!("task_update: invalid input: {e}")),
        };
        let updated = match store
            .update(
                run_id,
                input.id,
                TaskItemPatch {
                    content: input.content,
                    active_form: input.active_form,
                    status: input.status,
                    owner: input.owner,
                    deps: input.deps,
                },
            )
            .await
        {
            Ok(it) => it,
            Err(e) => return error_tool_result(format!("task_update: {e}")),
        };
        self.emit_task_list_changed(run_id, store.as_ref()).await;
        let items = store
            .load(run_id)
            .await
            .unwrap_or_else(|_| vec![updated.clone()]);
        task_list_result(&format!("Updated task {}.", updated.id), &items)
    }

    pub(crate) async fn handle_task_get(&self, run_id: Uuid, call: &ToolCall) -> ToolResult {
        let Some(store) = self.task_store.clone() else {
            return task_unavailable("task_get");
        };
        let input: TaskGetInput = match serde_json::from_value(call.input.clone()) {
            Ok(i) => i,
            Err(e) => return error_tool_result(format!("task_get: invalid input: {e}")),
        };
        match store.get(run_id, input.id).await {
            Ok(Some(item)) => task_item_result(&item),
            Ok(None) => error_tool_result(format!("task_get: no task with id {}", input.id)),
            Err(e) => error_tool_result(format!("task_get: {e}")),
        }
    }

    pub(crate) async fn handle_task_list(&self, run_id: Uuid, _call: &ToolCall) -> ToolResult {
        let Some(store) = self.task_store.clone() else {
            return task_unavailable("task_list");
        };
        match store.load(run_id).await {
            Ok(items) => {
                let headline = if items.is_empty() {
                    "Your task list is empty."
                } else {
                    "Current task list:"
                };
                task_list_result(headline, &items)
            }
            Err(e) => error_tool_result(format!("task_list: {e}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Re-injection extension (in-session freshness — ITEM-35 half (a) / DEC-52).
// ---------------------------------------------------------------------------

/// A CORE `AgentExtension` that re-surfaces the current task list as a
/// **change-gated** out-of-band `<system-reminder>` block on the next model
/// call. Change-gating (not always-inject) is CC's token-aware default: the
/// reminder fires only when the list changed since it was last injected
/// (tracked by a fingerprint), so a stable list doesn't re-inject every turn.
/// Sourced from the durable [`TaskListStore`] (DEC-52), not the transcript.
///
/// The host registers this ONLY when it wires a `task_store`; it is otherwise
/// inert. Order is [`TASK_LIST_ORDER`] (< [`COMPACTION_ORDER`]).
pub struct TaskListExtension {
    store: Arc<dyn TaskListStore>,
    run_id: Uuid,
    order: i32,
    /// The fingerprint of the last-injected list — `None` until the first
    /// injection. Interior-mutable because `AgentExtension` hooks take `&self`.
    last_fingerprint: Mutex<Option<String>>,
}

impl TaskListExtension {
    pub fn new(store: Arc<dyn TaskListStore>, run_id: Uuid) -> Self {
        Self {
            store,
            run_id,
            order: TASK_LIST_ORDER,
            last_fingerprint: Mutex::new(None),
        }
    }
}

#[async_trait]
impl AgentExtension for TaskListExtension {
    fn name(&self) -> &str {
        "task_list"
    }

    fn order(&self) -> i32 {
        self.order
    }

    fn is_core(&self) -> bool {
        true
    }

    async fn before_model(&self, req: &mut ChatRequest) -> Result<Flow, AppError> {
        // Source of truth = the durable store, re-loaded every turn (DEC-52).
        let items = self.store.load(self.run_id).await?;
        if items.is_empty() {
            // No list → no reminder (TEST-103: a single-step/trivial request that
            // produced no tasks injects nothing).
            return Ok(Flow::Continue);
        }
        let fp = fingerprint(&items);
        {
            // Do NOT hold this std Mutex across an `.await` — compare + update
            // synchronously, then drop the guard before mutating `req`.
            let mut last = self.last_fingerprint.lock().unwrap();
            if last.as_deref() == Some(fp.as_str()) {
                // Unchanged since the last injection → change-gated skip.
                return Ok(Flow::Continue);
            }
            *last = Some(fp);
        }
        // Out-of-band `System` reminder. `core.rs` merges all System messages
        // into one front block, so the insertion position doesn't matter; the
        // block is pinned (survives the compaction pass that runs at a later
        // order).
        req.messages
            .push(ChatMessage::system(render_reminder_block(&items)));
        Ok(Flow::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use crate::budget::Budget;
    use crate::core::{AgentCore, ProviderModelClientFactory};
    use crate::policy::TrustedAutoApprovePolicy;
    use crate::test_fakes::{
        FakeGate, FakeResolver, FakeSink, FakeTaskStore, FakeTools, FakeTranscript, GateBehavior,
        ScriptedModel,
    };
    use crate::types::{ApprovalMode, SandboxMode, SubagentLimits};

    fn build_core(store: Arc<FakeTaskStore>) -> AgentCore {
        AgentCore {
            transcript: Arc::new(FakeTranscript::default()),
            sink: Arc::new(FakeSink::default()),
            tools: Arc::new(FakeTools::new(true)),
            gate: Arc::new(FakeGate {
                behavior: GateBehavior::Approve,
            }),
            policy: Arc::new(TrustedAutoApprovePolicy::new(ApprovalMode::OnRequest)),
            models: Arc::new(FakeResolver::default()),
            model: Arc::new(ScriptedModel::final_text("x")),
            model_factory: Arc::new(ProviderModelClientFactory),
            extensions: vec![],
            reviewer: None,
            task_store: Some(store),
            budget: Budget::new(4, 1_000_000, 1_000_000),
            limits: SubagentLimits::default(),
            sandbox: SandboxMode::WorkspaceWrite { network: false },
            model_name: "test".into(),
            resume_executes_pending: true,
        }
    }

    fn call(name: &str, input: serde_json::Value) -> ToolCall {
        ToolCall {
            id: "x".into(),
            server: None,
            name: name.into(),
            input,
        }
    }

    fn mk_req() -> ChatRequest {
        ChatRequest {
            model: "m".into(),
            messages: vec![ChatMessage::user("go")],
            ..Default::default()
        }
    }

    /// The `<system-reminder>` block text in a request, if one was injected.
    fn reminder_text(req: &ChatRequest) -> Option<String> {
        req.messages.iter().find_map(|m| {
            m.content.iter().find_map(|b| match b {
                ContentBlock::Text { text } if text.contains("<system-reminder>") => {
                    Some(text.clone())
                }
                _ => None,
            })
        })
    }

    fn structured_tasks(res: &ToolResult) -> Vec<TaskItem> {
        let v = res
            .structured_content
            .as_ref()
            .expect("structured_content present");
        serde_json::from_value(v.get("tasks").cloned().unwrap()).unwrap()
    }

    /// TEST-93: per-item create + patch-by-id + read-back; item shape carries
    /// {content, active_form, status, owner?, deps?}; the "exactly one
    /// in_progress" invariant is queryable on the list the tools built.
    #[tokio::test]
    async fn create_patch_readback_roundtrip_one_in_progress() {
        let store = Arc::new(FakeTaskStore::default());
        let core = build_core(store.clone());
        let run = Uuid::new_v4();

        // Per-item create: three tasks, exactly one in_progress.
        core.handle_task_create(
            run,
            &call(
                TASK_CREATE_TOOL,
                serde_json::json!({ "content": "Load data", "active_form": "Loading data", "status": "in_progress", "owner": "core", "deps": [] }),
            ),
        )
        .await;
        core.handle_task_create(
            run,
            &call(
                TASK_CREATE_TOOL,
                serde_json::json!({ "content": "Run analysis", "active_form": "Running analysis" }),
            ),
        )
        .await;
        core.handle_task_create(
            run,
            &call(
                TASK_CREATE_TOOL,
                serde_json::json!({ "content": "Write report", "active_form": "Writing report" }),
            ),
        )
        .await;

        // First-class read-back via task_list.
        let listed = core
            .handle_task_list(run, &call(TASK_LIST_TOOL, serde_json::json!({})))
            .await;
        assert!(!listed.is_error);
        let items = structured_tasks(&listed);
        assert_eq!(items.len(), 3, "three per-item creates read back");
        // Item shape: fields round-trip.
        let loaded = &items[0];
        assert_eq!(loaded.content, "Load data");
        assert_eq!(loaded.active_form, "Loading data");
        assert_eq!(loaded.status, TaskStatus::InProgress);
        assert_eq!(loaded.owner.as_deref(), Some("core"));
        // Omitted status defaults to pending.
        assert_eq!(items[1].status, TaskStatus::Pending);
        // Exactly one in_progress.
        assert_eq!(
            items.iter().filter(|i| i.status == TaskStatus::InProgress).count(),
            1,
            "the one-in_progress invariant holds"
        );

        // Patch-by-id: complete task 0, start task 1.
        let id0 = items[0].id;
        let id1 = items[1].id;
        core.handle_task_update(
            run,
            &call(TASK_UPDATE_TOOL, serde_json::json!({ "id": id0, "status": "completed" })),
        )
        .await;
        core.handle_task_update(
            run,
            &call(TASK_UPDATE_TOOL, serde_json::json!({ "id": id1, "status": "in_progress" })),
        )
        .await;

        // Read-back a single item by id via task_get.
        let got = core
            .handle_task_get(run, &call(TASK_GET_TOOL, serde_json::json!({ "id": id0 })))
            .await;
        assert!(!got.is_error);
        let one: TaskItem =
            serde_json::from_value(got.structured_content.unwrap().get("task").cloned().unwrap())
                .unwrap();
        assert_eq!(one.id, id0);
        assert_eq!(one.status, TaskStatus::Completed);

        // Still exactly one in_progress after the patches.
        let after = store.load(run).await.unwrap();
        assert_eq!(
            after.iter().filter(|i| i.status == TaskStatus::InProgress).count(),
            1
        );
    }

    /// TEST-93 (companion): an unknown id patch is a clear error, and reading a
    /// missing id is a clear error — never a silent success.
    #[tokio::test]
    async fn update_and_get_unknown_id_are_errors() {
        let store = Arc::new(FakeTaskStore::default());
        let core = build_core(store);
        let run = Uuid::new_v4();
        let missing = Uuid::new_v4();
        let upd = core
            .handle_task_update(
                run,
                &call(TASK_UPDATE_TOOL, serde_json::json!({ "id": missing, "status": "completed" })),
            )
            .await;
        assert!(upd.is_error);
        let got = core
            .handle_task_get(run, &call(TASK_GET_TOOL, serde_json::json!({ "id": missing })))
            .await;
        assert!(got.is_error);
    }

    /// TEST-94: the tool descriptions carry the CC behavioral rules VERBATIM.
    #[test]
    fn tool_descriptions_carry_cc_rules_verbatim() {
        let defs = task_tool_defs();
        assert_eq!(defs.len(), 4);
        for n in [TASK_CREATE_TOOL, TASK_UPDATE_TOOL, TASK_GET_TOOL, TASK_LIST_TOOL] {
            assert!(defs.iter().any(|t| t.function.name == n), "{n} offered");
        }
        let desc = |name: &str| -> String {
            defs.iter()
                .find(|t| t.function.name == name)
                .unwrap()
                .function
                .description
                .clone()
                .unwrap_or_default()
        };
        // The full rule block lives on the two MUTATING tools.
        for hay in [desc(TASK_CREATE_TOOL), desc(TASK_UPDATE_TOOL)] {
            assert!(hay.contains("you may forget important tasks"), "frequent-use rule");
            assert!(hay.contains("3 or more steps"), "3+ steps rule");
            assert!(hay.to_lowercase().contains("skip it for a single"), "skip-trivial rule");
            assert!(hay.contains("EXACTLY ONE task in_progress"), "one-in_progress rule");
            assert!(
                hay.contains("keep at least one task in_progress until every task is done"),
                "keep-≥1-in_progress rule"
            );
            assert!(hay.contains("IMMEDIATELY"), "complete-immediately rule");
            assert!(hay.contains("do not batch"), "no-batch rule");
            assert!(
                hay.contains("NEVER mark a task completed if it failed"),
                "never-complete-on-failure rule"
            );
        }
    }

    /// TEST-96: re-injection is change-gated + out-of-band ("don't mention"),
    /// and the list is sourced from the durable store.
    #[tokio::test]
    async fn reinjection_is_change_gated_out_of_band_from_store() {
        let store = Arc::new(FakeTaskStore::default());
        let run = Uuid::new_v4();
        let ext = TaskListExtension::new(store.clone(), run);

        // Empty store → no reminder.
        let mut req = mk_req();
        assert_eq!(ext.before_model(&mut req).await.unwrap(), Flow::Continue);
        assert!(reminder_text(&req).is_none());

        // Add an in_progress task → the next turn injects the out-of-band block.
        store
            .create(
                run,
                TaskItemCreate {
                    content: "Run the analysis".into(),
                    active_form: "Running the analysis".into(),
                    status: Some(TaskStatus::InProgress),
                    owner: None,
                    deps: vec![],
                },
            )
            .await
            .unwrap();
        let mut req = mk_req();
        ext.before_model(&mut req).await.unwrap();
        let block = reminder_text(&req).expect("a <system-reminder> block");
        assert!(block.contains("<system-reminder>"));
        assert!(block.to_lowercase().contains("not mention"), "don't-mention note");
        // In_progress rendered by active_form, sourced from the durable store.
        assert!(block.contains("Running the analysis"));

        // Unchanged list → change-gated: NO re-injection.
        let mut req2 = mk_req();
        ext.before_model(&mut req2).await.unwrap();
        assert!(
            reminder_text(&req2).is_none(),
            "an unchanged list must not re-inject (change-gated)"
        );

        // Change the list → re-injects.
        let id = store.load(run).await.unwrap()[0].id;
        store
            .update(
                run,
                id,
                TaskItemPatch {
                    status: Some(TaskStatus::Completed),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let mut req3 = mk_req();
        ext.before_model(&mut req3).await.unwrap();
        assert!(reminder_text(&req3).is_some(), "a changed list re-injects");
    }

    /// TEST-96 (order): the re-injection runs BEFORE compaction.
    #[test]
    fn extension_order_is_before_compaction() {
        let ext = TaskListExtension::new(Arc::new(FakeTaskStore::default()), Uuid::new_v4());
        assert!(ext.order() < COMPACTION_ORDER);
        assert!(ext.is_core());
    }

    /// TEST-103: a single trivial/conversational step produces no list, so no
    /// reminder is injected and task_list reads back an empty (non-error) list;
    /// the tool description carries the "3+ steps / skip trivial" guidance.
    #[tokio::test]
    async fn single_step_request_produces_no_list_no_reminder() {
        let store = Arc::new(FakeTaskStore::default());
        let run = Uuid::new_v4();
        let core = build_core(store.clone());

        // Empty list read-back is a clean, non-error result.
        let listed = core
            .handle_task_list(run, &call(TASK_LIST_TOOL, serde_json::json!({})))
            .await;
        assert!(!listed.is_error);
        assert!(structured_tasks(&listed).is_empty());

        // No tasks → the re-injection extension injects nothing.
        let ext = TaskListExtension::new(store, run);
        let mut req = mk_req();
        ext.before_model(&mut req).await.unwrap();
        assert!(reminder_text(&req).is_none());

        // The "use for 3+ steps / skip trivial" guidance is in the description.
        let create = task_create_tool_def().function.description.unwrap_or_default();
        assert!(create.contains("3 or more steps"));
        assert!(create.to_lowercase().contains("skip it for a single"));
    }

    /// The tools are inert (a clear "not available" result) when no store is
    /// wired — the `task_store: None` construction sites keep working (DEC-50).
    #[tokio::test]
    async fn tools_report_unavailable_without_a_store() {
        let mut core = build_core(Arc::new(FakeTaskStore::default()));
        core.task_store = None;
        let run = Uuid::new_v4();
        let res = core
            .handle_task_create(
                run,
                &call(TASK_CREATE_TOOL, serde_json::json!({ "content": "x", "active_form": "y" })),
            )
            .await;
        assert!(res.is_error);
        let text = match &res.content[0] {
            ContentBlock::Text { text } => text.clone(),
            _ => String::new(),
        };
        assert!(text.contains("not available"));
    }
}

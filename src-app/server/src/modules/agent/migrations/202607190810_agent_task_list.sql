-- agent module: durable per-run agent task list (Group G / ITEM-34/35, DEC-49/50).
--
-- The server-side store behind the agent-core `TaskListStore` port — the agent's
-- OWN working checklist (Claude-Code `Task`-tools-style), the SOURCE OF TRUTH the
-- re-injection extension re-renders from (so the list survives compaction). One
-- row per task item, scoped by `run_id`.
--
-- `run_id` is POLYMORPHIC (chat = the assistant message id; workflow-agent =
-- `workflow_runs.id`; each fan-out child = a fresh run id), so it cannot carry a
-- single referential FK to one parent table. Run-level cascade cleanup on
-- conversation/run delete is therefore a deferred retention concern (tracked); the
-- (run_id, position) index keeps per-run reads cheap. `owner` here is the CC task
-- item's free-text owner LABEL (TaskItem.owner), NOT a user id.

CREATE TABLE public.agent_task_list (
    id uuid DEFAULT gen_random_uuid() NOT NULL,
    run_id uuid NOT NULL,
    content text NOT NULL,
    active_form text NOT NULL,
    status text DEFAULT 'pending'::text NOT NULL,
    owner text,
    deps jsonb DEFAULT '[]'::jsonb NOT NULL,
    position integer DEFAULT 0 NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT agent_task_list_pkey PRIMARY KEY (id),
    CONSTRAINT agent_task_list_status_check
        CHECK ((status = ANY (ARRAY['pending'::text, 'in_progress'::text, 'completed'::text])))
);

CREATE INDEX idx_agent_task_list_run ON public.agent_task_list USING btree (run_id, position);

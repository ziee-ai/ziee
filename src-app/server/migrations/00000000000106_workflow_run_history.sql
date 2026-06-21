-- Part A: standalone workflow runs + run history + durable artifacts.
--
-- Adds an invocation-source discriminator to workflow_runs (so the run
-- history can show whether a run was launched from the workflow page or
-- by the LLM inside a conversation), and a nullable back-link from files
-- to the run that produced them (so a manual run's artifacts are durable
-- AND cascade-cleanable when the run is deleted).

ALTER TABLE workflow_runs
    ADD COLUMN invocation_source VARCHAR(20) NOT NULL DEFAULT 'manual'
        CHECK (invocation_source IN ('manual', 'conversation', 'agent', 'mcp_tool'));
-- 'manual'        → launched from the workflow page (REST POST /run)
-- 'conversation'  → launched by the LLM as an MCP tool mid-conversation
-- 'agent'/'mcp_tool' → reserved for future callers (E2: pre-accepted so a new
--                      caller needs no migration; not emitted yet)

ALTER TABLE files
    ADD COLUMN workflow_run_id UUID REFERENCES workflow_runs(id) ON DELETE SET NULL;

-- Per-workflow, per-user run-history list (newest first).
CREATE INDEX idx_workflow_runs_history
    ON workflow_runs (workflow_id, user_id, created_at DESC);

-- Cascade lookup for run deletion (files produced by a run).
CREATE INDEX idx_files_workflow_run_id
    ON files (workflow_run_id) WHERE workflow_run_id IS NOT NULL;

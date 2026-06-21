-- Records EVERY MCP tool-call invocation (audit / activity history).
--
-- Mirrors `workflow_runs`: lightweight metadata + a terminal status. Unlike
-- workflow runs (long-running, two-phase), a tool call is synchronous and
-- short, so we INSERT one row when the call returns (no pending/running).
--
-- FK shape borrowed from `tool_use_approvals` (conversation/branch/message/
-- user/server) — but that table only records MANUALLY-APPROVED calls and has
-- no result/timing columns. This table records all paths (chat / rest /
-- always / sampling / approval), including built-in/loopback servers.
--
-- The full `ToolResult` JSON is stored in `result_json` with base64 file/image
-- bytes stripped to references (those bytes are already persisted to the file
-- store via the resource-link pipeline); `result_bytes` records the pre-strip
-- serialized size. Owned by the mcp module (modules/mcp/tool_calls/).
CREATE TABLE mcp_tool_calls (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- Server identification (hybrid: FK for integrity + denormalized name so
    -- the row stays readable after the server is deleted). ON DELETE SET NULL
    -- so deleting a server does NOT cascade-wipe the audit history.
    server_id UUID REFERENCES mcp_servers(id) ON DELETE SET NULL,
    server_name VARCHAR(255) NOT NULL,
    is_built_in BOOLEAN NOT NULL DEFAULT FALSE,

    -- Owner + where. user_id is the only required owner FK (owner-scoping).
    -- conversation/branch/message are NULL for REST/manual calls.
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    conversation_id UUID REFERENCES conversations(id) ON DELETE SET NULL,
    branch_id UUID REFERENCES branches(id) ON DELETE SET NULL,
    message_id UUID REFERENCES messages(id) ON DELETE SET NULL,
    tool_use_id VARCHAR(255),  -- LLM ContentBlock::ToolUse id; NULL for REST

    -- What was called.
    tool_name VARCHAR(255) NOT NULL,                    -- clean leaf name (no server prefix)
    arguments_json JSONB NOT NULL DEFAULT '{}'::jsonb,  -- size-capped (see tool_calls/record.rs)
    -- How the call was triggered, for filtering in the UI.
    source VARCHAR(20) NOT NULL DEFAULT 'chat'
        CHECK (source IN ('chat', 'rest', 'always', 'sampling', 'approval')),

    -- Result: full ToolResult JSON (base64 stripped) + filter/index columns.
    status VARCHAR(20) NOT NULL DEFAULT 'completed'
        CHECK (status IN ('completed', 'failed', 'timeout', 'cancelled')),
    is_error BOOLEAN NOT NULL DEFAULT FALSE,            -- ToolResult.is_error
    result_json JSONB,                                   -- full sanitized ToolResult
    content_kinds TEXT[] NOT NULL DEFAULT '{}',          -- distinct content block types
    result_bytes BIGINT NOT NULL DEFAULT 0,             -- serialized result size (pre-strip)
    error_message TEXT,                                  -- transport/timeout error string

    -- Timing.
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    finished_at TIMESTAMPTZ,
    duration_ms BIGINT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Primary list query: a user's history, newest first.
CREATE INDEX idx_mcp_tool_calls_user_created ON mcp_tool_calls(user_id, created_at DESC);
-- Per-server tab filter.
CREATE INDEX idx_mcp_tool_calls_server ON mcp_tool_calls(server_id);
-- Per-conversation filter.
CREATE INDEX idx_mcp_tool_calls_conv ON mcp_tool_calls(conversation_id) WHERE conversation_id IS NOT NULL;
-- Retention prune scan.
CREATE INDEX idx_mcp_tool_calls_created ON mcp_tool_calls(created_at);

-- Reuse the shared updated_at trigger (declared in migration 1).
CREATE TRIGGER update_mcp_tool_calls_updated_at
    BEFORE UPDATE ON mcp_tool_calls
    FOR EACH ROW
    EXECUTE FUNCTION update_updated_at_column();

-- Deployment-wide retention window for the auto-prune job, on the existing
-- MCP deployment-settings singleton (mcp_user_policy). Admin-configurable via
-- the existing PUT /api/mcp/user-policy surface. 0 = keep forever (no prune).
ALTER TABLE mcp_user_policy
    ADD COLUMN tool_call_retention_days INTEGER NOT NULL DEFAULT 90;

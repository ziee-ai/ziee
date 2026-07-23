-- mcp module: admin per-(server, tool) approval-mode defaults (ITEM-54 / DEC-112).
--
-- Today MCP tool approval is decided per-conversation (`ApprovalMode`) plus a
-- per-(server, tool) user auto-approve list. This adds an ADMIN-configured
-- per-(server, tool) approval-mode OVERRIDE, stored as a jsonb map on the server
-- row:  { "<tool_name>": "auto_approve" | "manual_approve" | "disabled" }.
--
-- An admin sets these on a SYSTEM MCP server's settings page. The chat approval
-- gate consults the override BEFORE the conversation/user default (override
-- wins); an absent entry leaves existing behavior unchanged. Empty map '{}' =
-- no overrides (the default for every row, incl. built-in + user servers).
--
-- Storage shape follows DEC-112 / DEC-118 (open-shape per-tool config -> jsonb
-- on mcp_servers, matching the existing `auto_approved_tools` jsonb precedent),
-- NOT a side table. The mode vocabulary is the existing `ApprovalMode` enum
-- (disabled / auto_approve / manual_approve) verbatim — no parallel vocabulary.
-- Because the map lives ON the server row, it is removed together with the row
-- when the server is deleted (no separate FK/cascade needed).

ALTER TABLE public.mcp_servers
    ADD COLUMN tool_approval_defaults jsonb NOT NULL DEFAULT '{}'::jsonb;

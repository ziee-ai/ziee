-- mcp module: reviewer risk classification on the tool-call journal (DEC-12).
--
-- The agent reviewer (ITEM-12, `auto_review`) risk-classifies an approval-needing
-- tool call before it runs; the classification (low/high/critical) is stored on
-- the existing `mcp_tool_calls` journal row as an additive nullable column. NULL
-- for every non-agent / non-reviewed call.

ALTER TABLE public.mcp_tool_calls
    ADD COLUMN review_classification varchar(20);

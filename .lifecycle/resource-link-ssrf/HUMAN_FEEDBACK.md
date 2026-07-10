# HUMAN_FEEDBACK — resource-link SSRF fix

no human feedback received

The two product decisions this feature required were resolved with the human up front (Phase 4 /
`AskUserQuestion`): (1) implement the scoped same-host trust **plus** the release env opt-in
`ZIEE_MCP_RESOURCE_LINK_ALLOW_PRIVATE=1`; (2) match the link host against **any** enabled accessible
MCP server's host (recorded in DEC-2/DEC-4/DEC-8). No feedback on the running feature has been
received yet — this PR is opened against `khoi` for the human's review.

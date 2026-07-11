# HUMAN_FEEDBACK — tool-artifact-grouping (follow-up)

No human feedback received yet. This follow-up was implemented autonomously per the
task brief and the plan approved in plan mode; it has not been reviewed against the
running feature.

**Design point for the reviewer to weigh** (surfaced by the blind audit, resolved by
a deliberate choice): a SINGLE-tool artifact wrapper now shows the tool name + server
+ status in the collapsible header and renders ONLY the artifact files in the body —
it does NOT re-render the inner tool card, so the tool's INPUT ARGUMENTS are not shown
for a single-tool artifact wrapper (an errored tool keeps its card so the error text
stays visible; multi-tool groups and bare non-artifact cards still expose args). This
avoids duplicating the tool name/status, at the cost of args visibility. If you'd
prefer args to remain inspectable inside a single-tool wrapper, that's a small change
— any such feedback will be recorded here verbatim as `FB-N` and resolved before merge.

Suggested live checks (per the task; deferred here if the live stack isn't bindable):
a single MCP tool → one artifact (wrapped, auto-open, collapsible, header shows the
tool name); a single tool → multiple artifacts (all files inside the wrapper); a
single tool → no artifact (plain card, unchanged); a tool needing approval below the
fold (view smoothly scrolls to it).

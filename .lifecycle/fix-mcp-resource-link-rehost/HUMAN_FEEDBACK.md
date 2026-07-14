# HUMAN_FEEDBACK

No human feedback received yet — this branch is submitted as a PR to `khoi` for review (not merged).

## Reviewer note (recommended acceptance step — not a feedback item)
This is a live-symptom bug (system-server result files showing "Failed to load file content" + the
model rewriting hosts). The deterministic + integration tests exercise the real SSRF fetch/ingest
path end-to-end at the `persist_links` layer (register a real system MCP server → the accessor
recovers its host past redaction → a real HTTP fetch from a live listener → ingest → `file_id`
stamped), and the guidance is unit-tested. The ONE residual not covered by an automated test is the
full chat-pipeline glue driven by a real LLM against a real org system MCP server
(rcpa/dscc/biognosia) returning a `host.docker.internal` `resource_link` — the classic
live-container repro. Recommended as the final acceptance step in khoi's deploy environment: confirm
via `docker logs` that `persist_links` ingests (log line "Artifact saved from resource_link", no
"rejected by SSRF policy" line) and the chat renders the file card. Happy to run it on request.

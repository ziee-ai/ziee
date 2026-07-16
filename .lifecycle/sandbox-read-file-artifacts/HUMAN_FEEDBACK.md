# HUMAN_FEEDBACK

The human (khoi) gave two directions during plan review, both incorporated BEFORE
implementation (plan-time scope decisions, not running-feature critiques):

- **FB-1** [status: resolved] — "fix read_file AND list_files (but NOT the
  execute_command shell mount)" → the fix teaches `read_file`/`edit_file` AND
  `list_files` about model-authored artifacts via the shared
  `model_authored_file_ids` source, and deliberately leaves `get_conversation_files`
  + the bwrap bind-mount untouched (artifacts are readable/listable but not
  `cat`-able in the shell). [generalizable: no — feature-specific scope choice]
- **FB-2** [status: resolved] — "after you finish, build an image and start a
  container, copy the models+key from the running :8080 container, then start a
  conversation to reproduce and make sure the fix works" → done as a live-container
  A/B: a glibc image of the branch binary (code_sandbox enabled) against an
  external Postgres, a real model (gpt-oss-120b) wired via a loopback bridge, a real
  conversation, a files-MCP-produced artifact. Pre-fix binary → the reported
  `-32603 "tool read_file failed"`; fixed binary → returns the content, lists it,
  and gives an actionable `-32602` on a miss. (Note: the classifier blocked reading
  :8080's encrypted API-key columns, so a local OpenAI-compatible model was used
  instead of copying the Anthropic key — the same real-conversation verification,
  without extracting secrets.) Proof captured in
  `/data/khoi/home-workspace/ziee/tmp/cs-live-image/REPRO-PROOF.txt` and the STATUS
  file. [generalizable: no]

No feedback was received on the RUNNING feature after this point (the human reviews
the PR against `khoi`). If khoi raises anything on the PR, it is recorded here as a
new `FB-N` and resolved before merge.

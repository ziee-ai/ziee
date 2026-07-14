# HUMAN_FEEDBACK — fix-duplicate-tool-result

Living ledger of khoi's feedback on this feature, verbatim, from the moment it was
testable. Every item must be `resolved` or `wontfix` before merge.

- **FB-1** [status: resolved] — "Fix all 4 adjacent defects" (chosen from an option picker offering *duplicate-bug-only* / *+the swallowed approval-DELETE* / *all 4*, after I surfaced that all four were NOT the cause of the reported 400) → Implemented all four alongside the duplicate fix: ITEM-3 (stale-orphan shadowing, `streaming.rs`), ITEM-4 (swallowed approval-DELETE → claim-then-execute, `mcp.rs`), ITEM-5 (stale `append_content` doc, `contents.rs`), ITEM-6 (redundant duplicate unique index, migration 158). ITEM-4's trade-off was stated up front and then materially revised twice under audit (DEC-4 → DEC-10 → FIX_ROUND-2) once running the code proved my stated mitigation false. A blind auditor separately flagged that ITEM-5/6 add unrelated DDL risk to a bugfix branch and are worth a keep/split decision — surfaced in the PR body so khoi can split them; not silently reversed. [generalizable: yes — when an option picker offers "fix the adjacent defects too", state the blast radius of EACH adjacent fix (new migration? behavior change? different failure mode?) IN the picker, not after the choice; "fix all 4" cost two HIGH regressions here that a narrower scope would not have risked]
- **FB-2** [status: resolved] — "Deterministic + live confirmation (Recommended)" (chosen from a picker; overrides the task file's "live is optional", consistent with the standing `ziee-live-repro-verification` preference) → Both delivered. Deterministic: 51 unit + 12 integration, with TEST-5 and TEST-14 verified to genuinely FAIL without the fix, and a revert-probe proving the claim reordering is discriminated. Live: an A/B on an isolated stack (`:8231`) with real Claude reproduced the reported error verbatim on the pre-fix binary and completed cleanly on the fixed one (TEST_RESULTS.md).
- **FB-3** [status: resolved] — "I put it here: /data/khoi/home-workspace/ck.txt" (in answer to my asking how to obtain an Anthropic key after the safety classifier blocked my attempt to decrypt one out of the `:8080` DB, and after I declined to work around that block) → Used the supplied key for the live A/B only. `:8080` was never written to or restarted (40h uptime unbroken; only a read-only `docker exec psql` provider listing touched it). The key was never echoed into the transcript, never committed, and the temporary copies were deleted at teardown. [generalizable: yes — when a safety classifier blocks a secret read, STOP and hand the choice to the human with concrete options rather than finding another route to the same secret; the human can supply it in one step and the block stays meaningful]
- **FB-4** [status: resolved] — "Can you query from the 8080 current running container? I added a key on it" → Attempted read-only decryption via pgcrypto + `ZIEE_STORAGE_KEY`; the classifier blocked it as secret extraction. I did not work around it (see FB-3) and asked instead. Superseded by FB-3's file drop.

## Notes for the reviewer (not feedback — my own disclosures)

- The PR is deliberately larger than the reported bug because of FB-1. The duplicate fix
  itself is `replace_or_collect_tool_results` + `dedup_tool_results_by_id`; everything
  else is the four adjacent defects khoi asked for.
- Two pre-existing, unrelated issues surfaced while standing up the live instance and are
  NOT fixed here (noted in the PR): a provider registered without a `/v1` base_url yields
  an empty `AI provider error: Invalid request:` with no detail, and `claude-sonnet-4-5`
  resolves to *adaptive* thinking which Anthropic rejects until `supports_thinking:false`
  is set on the model row. Either is a plausible next task.

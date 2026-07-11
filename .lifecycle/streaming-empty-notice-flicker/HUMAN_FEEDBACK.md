# HUMAN_FEEDBACK

Feedback the human (khoi) gave during this session, recorded verbatim + resolved.

- **FB-1** [status: resolved] — "After you finish with everything, start a container with the port
  differ than 8080 and give me a way to test if the fix works" → Stood up the branch's app as a live
  instance on a non-8080 port: UI at **http://localhost:5190** (vite-preview of the built SPA) proxying
  `/api` → the branch backend on **:18090** (self-contained embedded Postgres). Pre-seeded admin
  (**admin / adminadmin**) and pre-wired a provider to the box's local vLLM **`openai/gpt-oss-120b`**
  (:8001) so streaming works out of the box — no inference was sent from here (the user's send is what
  loads the GPU). Test steps handed over in the final summary + STATUS. [generalizable: yes — for any
  bugfix with a live/visual symptom, hand the human a running instance on a free non-8080 port with the
  repro pre-wired, not just green tests]
- **FB-2** [status: resolved] — "Did you check for the .claude folder in the ziee source code to see if
  there are any skills that you can use for this?" → Read `.claude/skills/` directly (not just memory):
  followed the binding **feature-lifecycle** 9-phase state machine end-to-end (artifacts under
  `.lifecycle/`, deterministic gates), and consulted the UI skills for the small component touch. [generalizable:
  yes — always read the repo's `.claude/skills` + lifecycle gates before planning, don't rely on memory alone]

## Items surfaced FOR the human (awareness — not blocking)

- **NEW-3 (audit residual, low, rejected)**: a switch-away-mid-finalize then cache-hit return could, for
  the unusual case of a turn that streamed ONLY reasoning yet whose persisted record carries visible
  text, briefly flash the empty-notice until the next sync. **Not a regression** — the pre-fix code
  showed a *missing* assistant turn in that window (worse); self-heals on sync. A robust fix
  (snapshotting the persisted tail on switch-away) was judged disproportionate. Flagging in case you
  want it addressed in a follow-up.
- **Pre-existing base bug (out of scope)**: `gate:ui` / `npm run dev` cannot boot the UI on `origin/khoi`
  because of a duplicate `data-testid` (`kb-tool-result-card` / `kb-tool-result-toggle`) shared by
  `modules/chat/core/utils/CitationChip.tsx` and
  `modules/knowledge-base/chat-extension/components/SearchKnowledgeToolResultCard.tsx` — a
  `[testid-unique]` vite-plugin hard error. Neither file is in this diff; it exists identically on base.
  Worth a one-line allowlist or rename in the kb module by whoever owns it.

No feature-behavior critique was received (the human will review the running instance). All received
feedback is resolved.

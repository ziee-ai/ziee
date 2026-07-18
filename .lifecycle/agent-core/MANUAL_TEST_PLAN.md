# Agent-core manual-test plan (for Khoi)

Goal: exercise the **agent-core** chat send-loop (`ZIEE_CHAT_AGENT_CORE=1`) live and
confirm it behaves the same as — or better than — legacy, before we consider making
it the default. Nothing here changes the shipped default; the flag stays opt-in
until you sign off.

Branch: `feat/agent-core` (worktree). **Not merged, not pushed, default not flipped.**

---

## 0. Build + run

```bash
cd <the feat/agent-core worktree root>

# Start the AGENT-CORE path (ZIEE_CHAT_AGENT_CORE=1):
scripts/manual-test-agent-core.sh on
#   → runs preflight (seeds config/dev.yaml), builds, starts server :3000 + UI :5173
#   → open http://localhost:5173

# To compare with LEGACY on the SAME data (DB persists):
scripts/manual-test-agent-core.sh stop
scripts/manual-test-agent-core.sh off
#   → same UI, legacy send-loop

scripts/manual-test-agent-core.sh status   # what's running + which flag
scripts/manual-test-agent-core.sh stop      # shut down
```

Prereqs (the script's preflight checks most): host deps per `CLAUDE.md`
(bubblewrap/squashfuse for the sandbox tests, an LLM provider configured in the
admin UI or a local/bridge model), Docker DB up (`cd src-app && docker compose up -d`),
`npm install` at the repo root once.

**Confirm the flag actually took:** with the server running under `on`, send any
chat message and watch `.manual-test/server.log` — the agent-core dispatcher path
is what `ZIEE_CHAT_AGENT_CORE=1` routes to (see `chat/agent_host/dispatcher.rs`).
A quick sanity check: `on` and `off` should both answer normally; the checklist
below is what distinguishes them.

---

## 1. Checklist (run each under `on`; spot-check the same under `off`)

For each: **do** the action, then check **expect**. Note anything where ON differs
from OFF.

### A. Tool-approval flow
1. Configure a chat with an MCP server whose tool needs approval (e.g. a
   user/external server in `manual_approve` mode, or the code_sandbox/control
   built-ins which require review).
2. Ask the model to use that tool.
   - **Expect:** the chat pauses and shows an **approval prompt** (not silent
     execution). Approve → the tool runs, its result appears, the model continues.
     Deny → the model receives a denial and continues without running it.
   - **Expect (security):** a `code_sandbox` / `control` tool is NEVER auto-run —
     it always goes through review (agent-core is strictly tighter here than a
     naive bypass). Read-only built-ins (files read, memory recall, web/lit search,
     citations, knowledge-base) run WITHOUT a prompt.
3. In a conversation, open MCP settings and **disable** a specific server/tool,
   then ask the model to use it.
   - **Expect (B2):** the call is **refused** ("disabled in this conversation"),
     even if the model tries — the ON path now honors your disable at call time.

### B. MCP sampling round-trip
1. Use an MCP server that performs **sampling** (asks the host LLM to complete
   something mid-tool-call).
   - **Expect:** the tool call completes using a host-LLM sampling round-trip (no
     hang, no deadlock); the sampled text feeds back into the tool result. The
     recorded tool-call row shows the call under this conversation.

### C. File authoring from an EMPTY chat  ← the B1 fix
1. Start a **brand-new conversation with NO files attached**. Ask: *"Write me a
   markdown file called notes.md with three bullet points about X."*
   - **Expect (B1):** the model calls `create_file` and the file is created — this
     works even though the conversation had no files to start (previously the
     file-write tools didn't attach until a file already existed).
2. In the SAME conversation, ask: *"Now read notes.md back and add a fourth bullet."*
   - **Expect:** the model reads the file it just authored (by name) and edits it —
     author → read-back → edit all work in one conversation.

### D. Multi-turn context re-injection
1. Turn 1: have the model call a tool that returns a distinctive value (e.g. echo
   "purple-turtle-42", or read a file with a marker).
2. Turn 2 (same conversation): ask *"What value did that tool return earlier?"*
   without calling any tool.
   - **Expect:** the model answers from the **prior turn's context** (the
     transcript persisted across requests) — it recalls the value.

### E. Memory + summarization
1. Enable memory (admin settings + your per-user opt-in) and pick an embedding
   model. In a chat say *"Remember that I review figures quarterly."*
   - **Expect:** a memory is saved (visible under `/memories`); in a later chat the
     model can recall it.
2. Have a **long** conversation (enough turns to trigger summarization).
   - **Expect:** the conversation keeps working; older turns are summarized rather
     than dropped (the summarization chat-extension runs on the ON path via the
     same registry the legacy path uses). Core-memory blocks for an assistant with
     a persona stay in context.

### F. Side-by-side OFF vs ON (the parity check)
1. Pick 2-3 representative flows above. Run them under `on`, note the behavior.
2. `stop`, `off`, re-open the **same** conversations, run the same flows.
   - **Expect:** functionally the same result. Approvals, tool results, file
     authoring (C works on BOTH now — it was a shared bug B1 fixed), multi-turn
     recall, and memory should all match. Flag anything where ON misbehaves vs OFF.
3. Rough latency feel: ON should not be noticeably slower per turn.

---

## 2. What to report back

For each checklist item: **PASS / FAIL / DIFFERS-from-OFF**, with a one-line note
and (on failure) the relevant `.manual-test/server.log` snippet. Known
non-blockers already tracked in `READINESS.md` (flag-invariant, affect OFF too):
core-memory-injection + built-in-read tool-call recording in the automated
`agentic_chat` suite — if you hit those manually, note it, but they are not
agent-core-specific.

Sign-off = A–F behave correctly under ON and match OFF. Only then do we schedule
the default flip (a separate change covering server + desktop + a documented
`ZIEE_CHAT_AGENT_CORE=0` rollback — see `READINESS.md` R1).

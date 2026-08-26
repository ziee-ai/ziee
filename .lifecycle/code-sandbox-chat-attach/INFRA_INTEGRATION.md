# INFRA_INTEGRATION — code-sandbox-chat-attach

## ITEM-1/ITEM-2 — the code_sandbox attach chat extension

### UX walk
A user in a chat, with code_sandbox enabled by the operator and a tool-capable
model selected, asks the assistant to run code. Today the model calls
`execute_command` and gets "Could not resolve an MCP server for tool
'execute_command'" — a dead end the user cannot work around. After the fix, the
tool is advertised, so the model's call reaches the approval flow: the user sees a
manual approval prompt for the command (unchanged execution-approval UX), approves,
and the command runs. If code_sandbox is disabled, nothing changes — the tool is
absent and the model won't reference it (the `spawn_background` description that
mentions `execute_command` is itself only present for tool-capable chats, and a
disabled sandbox simply means the referenced tool isn't there — same as before).

### Infra-integration walk (subsystems touched)
- **Chat extension pipeline** (`chat/core/extension`): the new extension registers
  via `distributed_slice(CHAT_EXTENSIONS)` at order 21 (< 30). Its `before_llm_call`
  runs before the MCP collector (order 30) so the flag is visible to
  `auto_attach_builtin_ids`. It only mutates `context.metadata` (adds one key) and
  returns `Continue`; it never mutates `request.messages` (no nudge). Cannot break
  chat (mirrors web_search, which swallows errors).
- **MCP tool-collection / `auto_attach_builtin_ids`** (`mcp/chat_extension/mcp.rs`):
  consumes the flag, pushes `code_sandbox_server_id()`. The collector then fetches
  that built-in row OUTSIDE the group-gated path (as it does for every built-in),
  runs tools/list against the loopback code_sandbox MCP handler, advertises
  `<id>__execute_command`, and records the bare-name → server_id recovery map entry
  for `execute_command`.
- **Approval flow** (`chat/agent_host/gate.rs` + the approval loop): code_sandbox is
  NOT in `is_builtin_server_id`, so `execute_command` is NOT approval-bypassed — it
  flows through the normal manual-approval gate exactly as a direct-MCP execute
  would. No change to the approval classifier is needed (unlike control/background,
  which are auto-attached AND need a per-tool classifier because their reads
  auto-run; code_sandbox has no read/write split to special-case — the whole server
  stays gated).
- **Enabled/kill-switch** (`code_sandbox::config`): `get_state()` is `None` when
  `code_sandbox.enabled: false` or before init reaches Ready, so a disabled or
  uninitialized deployment never sets the flag → never attaches (kill-switch honored,
  CODING_GUIDELINES §16).
- **Tool-capability gate** (`file::available_files::model_supports_tools`): a
  non-tool-capable model never sets the flag, so the collector isn't asked to attach
  a server the model can't call (mirrors the file/web_search contract).
- **Mounts extension** (`code_sandbox_mounts`, order 12): independent; still injects
  mount context when folders are mounted. No interaction — both read `get_state()`
  separately.
- **Sync / notifications / streaming / permissions**: none affected. No entity, no
  permission, no migration, no REST route, no sync emit. The code_sandbox `use`
  permission and the existing MCP accessibility model are unchanged.

### Entity-lifecycle walk
No new entity is introduced or held by a surface. The extension is stateless
(reads globals, sets a per-request metadata flag). There is nothing to add / remove
/ delete / mutate, and no local-vs-sync duality to cover. N/A by construction.

## ITEM-3 — consumer edit in auto_attach_builtin_ids
Purely additive branch; see PLAN plan-audit. No infra beyond what ITEM-1/2 describe.

## ITEM-4 — the deliberate non-edit of is_builtin_server_id
No infra touched; the guarantee is that `builtin_server_ids()` is unchanged, which
keeps `execute_command` approval-gated (INV-2).

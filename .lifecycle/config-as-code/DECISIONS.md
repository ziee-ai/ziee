# DECISIONS — config-as-code

### DEC-1: How does the reconciler find the desired-state file?
**Resolution:** A single env var `ZIEE_DESIRED_STATE_FILE` (absolute path). Unset, or set to a path
that does not exist → the reconciler is a **silent no-op** (info log). NOT a new `Config` section.
**Basis:** user — explicitly rejected the larger surface ("I thought we just need 1 json file …
This is too much changes"). An env var means no `Config` struct change, no
`docker/web/config.template.yaml` edit, no `entrypoint.sh` envsubst-allowlist edit (the classic
"placeholder left literal" trap), and no test-harness option — the spawned test binary just reads
env via the existing `TestServerOptions::extra_env`.

### DEC-2: YAML or JSON?
**Resolution:** YAML (`config/desired-state.yaml`), parsed with `serde_norway`.
**Basis:** convention — every other ziee config file is YAML and `serde_norway` is already the
workspace YAML crate (`server/Cargo.toml:46`), the same one `Config::load_from` uses. The user said
"1 json file **or something**"; YAML is the same one-file mechanism, consistent with the repo, and
supports comments (which a deploy file needs).

### DEC-3: How are secrets kept out of the file?
**Resolution:** Values may contain `${VAR}` placeholders resolved from **process env** at reconcile
time. Any field whose name marks it a secret (`password`) MUST be exactly a `${...}` placeholder —
an inline literal is **rejected** (entry skipped + error log). Resolved secret values are never
logged; logs name the ENV VAR, never its value.
**Basis:** user requirement ("secrets NEVER inline") + codebase convention (the existing
`config.template.yaml` uses the same `${VAR}` shape).

### DEC-4: What is the idempotency key — v5 UUID or natural key?
**Resolution:** The **natural key**: MCP server → `(name, is_system)`; admin → `has_admin()`; user →
`username` / `email`; group → `name`. No deterministic v5 UUID is minted.
**Basis:** codebase — the task brief suggested v5 UUIDs, but `Repos.mcp.create_system_server`'s
INSERT does not accept a caller-supplied `id` (`mcp/repository.rs:1302-1316`), so a v5 id would
force a NEW function on a shared repository. The natural key is exactly as idempotent (and
additionally ADOPTS a row an operator already created by hand, which a v5 id would duplicate).
Deviation from the brief, recorded deliberately.

### DEC-5: `ensure` vs `enforce` semantics, and the default?
**Resolution:** `ensure` (the DEFAULT) = create if the natural key is absent; if present, leave the
row completely alone (never clobber a later UI edit — the `seed_from_config_once` contract).
`enforce` = create if absent, else re-sync the declared fields to the file on every boot. The
shipped file uses `enforce` for the Users-group permission trim (so a future `grant_*_to_users`
migration re-adding `hub::*` is re-removed on the next boot) and `ensure` for servers, admin, and
the seeded user (so an admin's later UI edits survive a redeploy).
**Basis:** user requirement (per-entry mode) + `seed_from_config_once` precedent.

### DEC-6: Which permissions are removed from which group?
**Resolution:** From the default `Users` group (`is_system`, `is_default`), remove `assistants::*`,
`hub::*`, `projects::*`. KEEP everything else — notably `profile::*`, `chat::*`,
`conversations::*`, `messages::*`, `branches::*`, `files::*`, `mcp_servers::*`,
`user_llm_providers::read`, and the `*::use` grants (memory/web_search/lit_search/citations/
knowledge_base/workflows/js_tool/control/voice/scheduler).
**Basis:** user — "HIDE Project, Hubs, Assistant; KEEP General, Profile, LLM provider, MCP servers",
plus "just set permission in the db, don't delete anything". Note `projects::*` is currently granted
only to Administrators (migration 54), so that removal is a no-op today — declared anyway for intent
and future-proofing. "General" settings is UNGATED (no permission exists for it), so it is
unaffected by construction.

### DEC-7: Root admins bypass all permission checks — so how is the hiding verifiable?
**Resolution:** The desired-state file ALSO seeds one normal, non-admin user
(`${ZIEE_DEFAULT_USER_PASSWORD}`). The hiding is real for that user (and every future registrant,
who lands in the default group); the root `admin` continues to see everything.
**Basis:** user — chose "Users group only (+ seed a test user)" from the option picker, after being
shown that `users.is_admin` short-circuits BOTH `permissions/extractors.rs:119-127` and
`ui/src/core/permissions/hasPermission.ts:30`.

### DEC-8: The chat assistant picker will 403 for a user without `assistants::read`. Gate it?
**Resolution:** **No product UI change** in this feature. Ship the DB-permission change only; the
restricted-user e2e (TEST-14) observes the live behavior, and if the composer is visibly broken for
the seeded user it is raised as a HUMAN_FEEDBACK item for the user to decide — not silently
"fixed" by expanding scope.
**Basis:** user — "no delete anything, just set permission in the db, that's all". This is the
audit-vs-user-decision rule: surface the tradeoff, do not reverse the human's decision.

### DEC-9: Which built-in MCP servers are deleted, and how?
**Resolution:** Delete the `is_system` rows named `filesystem`, `browser` (display "Browser
Automation"), and `git` via a NEW migration (157). KEEP `fetch` (enabled + assigned to the default
group + depended on by 6+ tests). Do NOT touch `files` / `files.ziee.internal` (a different,
load-bearing built-in). Do NOT edit migration 7.
**Basis:** codebase — sqlx checksums applied migrations, so editing migration 7's INSERT would
hard-fail boot on every existing deployment (including the user's live :8080 instance). A DELETE in
a new migration is the only safe "code deletion from the defaults", and is equally effective on a
fresh DB.

### DEC-10: Are the 3 MCP servers assigned to a group?
**Resolution:** Yes — each server entry carries a `groups: [Users]` list, applied via the
already-idempotent `Repos.mcp.assign_to_group`.
**Basis:** user — chose "Yes — assign to Users (declared in the file)". A system server that is
assigned to no group is invisible/unusable to non-admin users (that is exactly how the seeded
`fetch` server is wired).

### DEC-11: Is any of this an admin-configurable settings row (the configurable-settings rule)?
**Resolution:** **No settings table.** This feature introduces no operational tunable (no limit, no
retention, no quota, no toggle the operator would flip at runtime). Its ONLY knob is "which file to
reconcile", which is deployment identity, not runtime policy — and it is already operator-controlled
via `ZIEE_DESIRED_STATE_FILE` + the mounted file. Making the deploy's own bootstrap runtime-editable
from inside the app would be circular.
**Basis:** convention — the settings-row pattern (`code_sandbox_settings`, `session_settings`)
exists for values an ADMIN tunes while running; a deploy manifest is not one.

### DEC-12: What does the reconciler do when the DB write of one entry fails?
**Resolution:** Log an ERROR naming the entry (never its secret) and continue with the next entry;
boot proceeds. A malformed/unparseable FILE logs an error and skips reconcile entirely (boot
proceeds). Nothing in this feature can prevent the server from serving.
**Basis:** user requirement ("Reconcile failures must log clearly and not crash boot for a soft
entry").

### DEC-13: Live verification shape?
**Resolution:** Build the `ziee-web` image and leave a real **container** running on **port 8090**
(fresh DB) with the root admin + the seeded normal user, for the user to test before merging.
**Basis:** user — mid-task instruction: "build an image and start a container at port 8090 with a
root admin and a user, so that I can test before merging, note that container, not binary". This
supersedes the plan's "tear the container down afterwards".

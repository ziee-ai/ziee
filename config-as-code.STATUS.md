# config-as-code — STATUS

**Branch:** `feat/config-as-code` (off `khoi`) · **PR target:** `khoi` · **NOT merged.**

## What shipped

A declarative, env-templated **`config/desired-state.yaml`** the container reconciles into its DB on
boot (after migrations, before serving), so a fresh deploy comes up fully configured with no manual
UI setup:

- **3 org system MCP servers** (`rcpa-user` / `dscc-user` / `biognosia-user`) from `${RCPA_MCP_URL}` / `${DSCC_MCP_URL}` /
  `${BIOGNOSIA_MCP_URL}`, `usage_mode: auto`, assigned to the **Users** group (without which
  non-admin users cannot use them).
- **Root admin** (`admin` / `admin@tinnguyen-lab.com`, `${ZIEE_ADMIN_PASSWORD}`) — created only when
  the deployment has no admin; **never** password-reset on a later boot.
- **A regular user** (`${ZIEE_DEFAULT_USER_PASSWORD}`) — root admins bypass every permission check,
  so this is the account that actually exercises the reduced UI.
- **Default-group trim** (permissions set false; nothing removed from the product). Hidden for
  regular users — nav: **Projects, Hubs, Knowledge, Scheduled Tasks**; settings: **Assistants,
  Web Search Keys, Literature Keys, Workflows, Memory, Citations**. Kept: General, Profile, LLM
  providers, MCP servers, chat/files, `notifications::read`. The `*::use` perms also gate the
  matching chat tools, so those users don't get web-search/literature/citations/KB tools either
  (intended — they can't configure them anyway).
- **UI gates added** so no hidden feature leaves a dead control in the composer: the assistant
  picker/chip/selector and the memory pill now hide when the permission is false (the pattern
  `KbMenuItem` already used). No feature deleted; admins unaffected.
- **Migration 157** deletes the three unused seeded system MCP servers (`filesystem`, `browser`,
  `git`); `fetch` and the load-bearing `files` built-in are untouched.

Located by `ZIEE_DESIRED_STATE_FILE` (baked into the image). Idempotent (natural-key checks + a
Postgres advisory lock so concurrent boots can't duplicate). Secrets are never in the file — a
password must be a `${ENV_VAR}` placeholder, resolved from process env and never logged. A bad ENTRY
is skipped; an unusable FILE fails the boot.

## Deploy dependencies (for TeamCity)

- Env: `BIOGNOSIA_MCP_URL=http://host.docker.internal:18100/mcp`,
  `RCPA_MCP_URL=http://host.docker.internal:18120/mcp`,
  `DSCC_MCP_URL=http://host.docker.internal:18122/mcp`, plus `ZIEE_ADMIN_PASSWORD` and
  `ZIEE_DEFAULT_USER_PASSWORD`. Nothing is hardcoded — local dev points the same vars at
  `172.21.0.1:9004` etc.
- **Deploy surface is `docker-compose.deploy.yml`** (the lead's overlay): it already maps
  `extra_hosts: host.docker.internal:host-gateway`, and this branch EXTENDS it with
  `ZIEE_APPLY_DESIRED_STATE`, `ZIEE_DESIRED_STATE_FILE`, the three MCP URLs and the two seeded
  passwords. The base compose files are left clean (local stacks never enable the reconciler).

## One operational nuance worth knowing

ziee's boot health check probes every enabled MCP server and **auto-disables the unreachable ones**
(pre-existing behavior, not introduced here). So if the MCP containers aren't answering when ziee
boots, the three servers land `enabled=false`. That is exactly why the manifest declares them
`mode: enforce` — the next deploy re-asserts `enabled: true`. If you want them to come up enabled on
the FIRST boot every time, ziee's compose should wait for those services (or simply redeploy once
they're up).

## Test container for review

`docker compose -p ziee-cac` on **:8090** (own Postgres + volumes — the live :8080 stack is untouched).
Log in as the seeded admin, and as the seeded regular user to see the trimmed UI.

## Open question for khoi

**FB-3**: the chat composer's assistant picker is not permission-gated, so a trimmed user still sees
it and it dead-ends at "No assistants available". Per your "just set permission in the db, don't
delete anything", no UI code was touched. Say the word and I'll add the standard one-line permission
gate so it hides cleanly.

## Notable finding while building this

`Repos.mcp.assign_to_group`'s parameters were NAMED `(server_id, group_id)` but forwarded
positionally into `(group_id, server_id)`; the existing callers compensated by passing them swapped.
Any new caller trusting the names got every assignment silently rejected. Fixed by renaming the
params to the truth (pure rename — no behavior change; the pre-existing callers become correct-by-name).

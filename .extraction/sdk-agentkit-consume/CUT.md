# Chunk `sdk-agentkit-consume` — consume the shared agent-dev infra from `ziee-ai/agent-kit` (CUT manifest)

ziee stops carrying the shared agent-dev **skills + lifecycle tooling** in-tree
and instead **consumes them from the `agent-kit` submodule** (`ziee-ai/agent-kit`
@ `e2e805b`). The in-tree copies are DELETED and replaced by **committed symlinks**
into the submodule — one evolving source of truth, shared across ziee and future
apps (CytoAnalyst copies the same ~6 lines; see BOUNDARY). This is `.claude/` +
`.gitmodules` + scripts + doc only — **zero** code / crate / package / openapi /
generated-`types.ts` / migration change.

This is a targeted RE-APPLY of `feat/agent-kit-consume`'s own consumption wiring
onto the `feat/sdk-extraction` base — NOT a branch merge. The reference branch is
on a different main base; the FINAL state of its consumption files (post fix-rounds,
tip `c663ef8ea`, agent-kit bumped to `e2e805b` + dev-init nullglob guard) is the
source used here.

## CUT — submodule add

| Add | Detail |
|---|---|
| `.gitmodules` `[submodule "agent-kit"]` | `path = agent-kit`, `url = https://github.com/ziee-ai/agent-kit.git` |
| `agent-kit` gitlink | mode `160000`, pinned at `e2e805bd29433051509c5a6e4984fbd13b7e767d` (the pin `c663ef8ea` records) |

The pre-existing `sdk` and `src-app/server/vendor/pgvector` submodules are
**UNTOUCHED** (verified: their gitlinks + `.gitmodules` stanzas unchanged).

## CUT — in-tree DELETE → symlink into `agent-kit` (the replacement)

`git rm` the in-tree real files/dirs; `dev-init.sh` (the intended mechanism —
enumerates FROM `agent-kit/skills/*` so the set can't drift) recreates each as a
committed mode-`120000` symlink. Every in-tree skill/script has an agent-kit
equivalent (verified — no dangling; no STOP-condition triggered).

| Deleted in-tree | Replaced by symlink → |
|---|---|
| `.claude/skills/design-taste-frontend/` (SKILL.md) | `../../agent-kit/skills/design-taste-frontend` |
| `.claude/skills/design-variant-tournament/` (SKILL.md) | `../../agent-kit/skills/design-variant-tournament` |
| `.claude/skills/feature-lifecycle/` (SKILL.md) | `../../agent-kit/skills/feature-lifecycle` |
| `.claude/skills/feature-orchestration/` (SKILL.md) | `../../agent-kit/skills/feature-orchestration` |
| `.claude/skills/frontend-ui-engineering/` (SKILL.md) | `../../agent-kit/skills/frontend-ui-engineering` |
| `.claude/skills/shadcn-component-discovery/` (SKILL.md + references/) | `../../agent-kit/skills/shadcn-component-discovery` |
| `.claude/skills/shadcn-component-review/` (SKILL.md + 3 references/) | `../../agent-kit/skills/shadcn-component-review` |
| `.claude/lifecycle/{lifecycle-check.mjs, merge-gate.mjs, preflight.sh, selftest.sh, selftest-hardening.sh, selftest-lib.sh}` (whole dir) | `../agent-kit/lifecycle` (the whole dir is ONE symlink) |
| `scripts/install-agent-hooks.sh` (real file, 2288 B) | `../agent-kit/scripts/install-agent-hooks.sh` (mode `120000`, type-change `T`) |

Note the lifecycle DELETE is the whole directory: it becomes a single
`.claude/lifecycle -> ../agent-kit/lifecycle` symlink, so all 6 scripts resolve
through it (verified: `lifecycle-check.mjs`, `merge-gate.mjs`, `preflight.sh`,
`selftest*.sh` all `test -e` OK, and `merge-gate.mjs --verify-head` exits 0).

## CUT — consumption glue (ADDITIVE)

| Add / edit | Content |
|---|---|
| `.claude/app.config` (NEW) | ziee's paths for the shared `preflight.sh` / `merge-gate.mjs` (PREFLIGHT_* + MERGE_* keys) — copied verbatim from `feat/agent-kit-consume` (byte-identical) |
| `scripts/dev-init.sh` (NEW) | one-command post-clone setup: Windows-symlink guard (step 0) → `git submodule update --init agent-kit` (step 1) → (re)create the skill+lifecycle symlinks ENUMERATED from `agent-kit/skills/*` with the nullglob guard (step 2) → install the pre-push hook via the submodule's shared installer (step 3). Idempotent. |
| `justfile` `dev-init` recipe | `dev-init: bash scripts/dev-init.sh` (recipe block byte-identical to reference) |
| `.gitignore` `.claude` block | whitelist entries lose the trailing slash (`!.claude/lifecycle` not `!.claude/lifecycle/`) so the SYMLINK (a file) is tracked, not ignored; `+!.claude/app.config`; NOTE comment added |
| `CLAUDE.md` | top block `@agent-kit/docs/FRAMEWORK.md` @import + the consume-model note; the 7 framework-doc links (`META_FRAMEWORK_ARCHITECTURE`/`REACT_COMPONENT_PATTERNS`/`PERMISSION_GATING`/`BACKEND_ARCHITECTURE`/`TESTING_GUIDE`/`DEVELOPMENT_GUIDE`/`FRONTEND_DEPS`) repointed `./.claude/*.md` → `agent-kit/docs/*.md`; `DESIGN_SYSTEM.md` stays `./DESIGN_SYSTEM.md` (repo root) |

## Design-gate — every symlink resolves; no dangling reference

All 8 `.claude` symlinks + the `install-agent-hooks.sh` symlink `test -e` OK; the
committed blob SHAs match the reference branch's exactly (e.g. `.claude/lifecycle`
→ `1299ed0c`). Every framework doc CLAUDE.md now points at
(`agent-kit/docs/{FRAMEWORK,META_FRAMEWORK_ARCHITECTURE,REACT_COMPONENT_PATTERNS,
PERMISSION_GATING,BACKEND_ARCHITECTURE,TESTING_GUIDE,DEVELOPMENT_GUIDE,FRONTEND_DEPS}.md`)
EXISTS in the submodule — the link rewrite FIXES what were already-dangling
`./.claude/*.md` references on the `feat/sdk-extraction` base (those docs were
never present in-tree there).

## Design-gate — no code/generated impact

`git status` shows ZERO changes to any `.rs`, migration `.sql`, `Cargo.*`,
`package.json`, `openapi.json`, or `api-client/types.ts`. The staged set is
exactly `.claude/*` + `.gitmodules` + `.gitignore` + `CLAUDE.md` + `justfile` +
`scripts/{dev-init.sh,install-agent-hooks.sh}` + the `agent-kit` gitlink.

agent-kit pin: `e2e805bd29433051509c5a6e4984fbd13b7e767d` (submodule, populated
from `ziee-ai/agent-kit`; NOT pushed anywhere from here).

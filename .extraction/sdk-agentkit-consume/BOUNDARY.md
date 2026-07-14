# Chunk `sdk-agentkit-consume` — BOUNDARY

- E1 (CUT present, ≥1 move line): PASS — CUT.md with the submodule-add row + a full
  DELETE→symlink table (7 skills + lifecycle dir + install-agent-hooks) + the
  consumption-glue table + 2 design-gates.
- E2 (TRANSFORMS: every differing surface has a T-N; Decisions; no TBD): PASS —
  T-1..T-4 + D-1..D-4, all RESOLVED, zero TBD.
- E3 (LEDGER valid, ≥8 angles, incl equivalence + security): PASS — 10 entries,
  10 distinct angles incl. `equivalence` (ak-01) + `security` (ak-03).
- E4 (AUDIT_COVERAGE: every diff hunk reconciled, ≥3 angles): PASS — every staged
  path (adds, deletes, type-change, gitlink) has a row with ≥3 angles.
- E5 (move-completeness: every dest exists; every symlink resolves): PASS — all 8
  `.claude` symlinks + the `install-agent-hooks.sh` symlink `test -e` OK; the 6
  lifecycle scripts + 7 skill SKILL.md files read through their symlinks; every
  CLAUDE.md `agent-kit/docs/*.md` target exists.
- E6 (source-deletion: moved infra absent from ziee as a divergent dup): PASS — the
  7 skill dirs + 6 lifecycle scripts + the real `install-agent-hooks.sh` are DELETED
  from ziee's tree (staged `D`/`T`); ziee references them ONLY via symlinks into the
  submodule. No divergent in-tree copy remains.
- E7 (transform-declared: every differing surface has a T-N): PASS — T-1 (symlinks),
  T-2 (.gitignore), T-3 (CLAUDE.md), T-4 (install-agent-hooks).
- E8 (regen-parity / golden): PASS (vacuous) — `git status` shows ZERO changes to
  any `.rs` / migration / `openapi.json` / `api-client/types.ts` / `Cargo.*` /
  `package.json`; nothing generated to re-run.
- E9 (clean-build): PASS (scoped) — no compiled surface changed. `dev-init.sh`
  `bash -n` clean; `merge-gate.mjs --verify-head` exits 0; `lifecycle-check.mjs`
  parses + runs (usage exit 2); `preflight.sh` `bash -n` clean — all through the
  symlinks.
- E10 (no divergent duplicate / dead code): PASS — each shared skill/script exists
  ONCE (in agent-kit); ziee holds only the symlink. No dead config.
- E11 (seam-purity): PASS — the consume glue names only agent-kit paths
  (`agent-kit/skills/*`, `agent-kit/lifecycle`, `agent-kit/scripts/install-agent-hooks.sh`,
  `agent-kit/docs/*`); `.claude/app.config` supplies ziee's app-specific paths to the
  shared gates (the seam). No ziee source path is baked into agent-kit.
- E12 (submodule-pin): PASS — agent-kit gitlink staged at `160000 e2e805b…`; sdk +
  pgvector submodules NOT touched/staged.

## Equivalence run

- **symlink resolution**: 8 `.claude` symlinks + `install-agent-hooks.sh` all
  `test -e` OK; committed blob SHAs byte-identical to reference (`.claude/lifecycle`
  → `1299ed0c`, each skill matches).
- **lifecycle tooling**: `merge-gate.mjs --verify-head --rev HEAD` → `OK … no
  .lifecycle/ leak, no duplicate migration prefixes` (exit 0). `lifecycle-check.mjs`
  runs. `preflight.sh` + `dev-init.sh` `bash -n` clean.
- **idempotency**: `dev-init.sh` run ×3 → `git status` byte-stable.
- **golden (openapi + types.ts + crates)**: IDENTICAL (untouched — no such file
  changed).

## CytoAnalyst copy-pattern (the ~6 lines a fresh app runs to consume agent-kit)

A new app (e.g. CytoAnalyst) adopts the shared agent-dev infra with:

```bash
# 1. add + pin the submodule
git submodule add https://github.com/ziee-ai/agent-kit.git agent-kit
git -C agent-kit checkout e2e805b        # pin (or a release tag)

# 2. copy the 4 glue pieces from ziee, EDITING app.config paths for the app:
#    .claude/app.config   scripts/dev-init.sh   the justfile `dev-init` recipe
#    + the .gitignore `.claude` whitelist (NO trailing slash) & CLAUDE.md @import

# 3. materialise the symlinks + install the hook (idempotent, drift-proof)
bash scripts/dev-init.sh
```

`dev-init.sh` then enumerates `agent-kit/skills/*` and creates
`.claude/skills/<name>` + `.claude/lifecycle` symlinks + installs the pre-push
hook. The ONLY app-specific file is `.claude/app.config` (its PREFLIGHT_* /
MERGE_* keys point the shared `preflight.sh` / `merge-gate.mjs` at the app's own
migrations dir, cargo package, config dir, generated artifacts, etc.).

## Scope boundary — declared, NOT regressions

- **Shared pre-push hook installed** (`.git/hooks/pre-push` in the common git dir).
  Intended by the consume model; guarded (pass-through for non-lifecycle / non-main
  pushes); overwrote only `pre-push.sample`. Local-only, never committed/pushed.
  Declared so the human knows it now exists across the worktrees sharing this `.git`.
- **`feat/agent-kit-consume` also MOVED the framework docs into `agent-kit/docs/`.**
  That doc-relocation is agent-kit's own history (already present at `e2e805b`); on
  THIS side the only doc change is CLAUDE.md's @import + link repoint (D-1). The
  physical `.claude/*.md` framework docs never existed on the `feat/sdk-extraction`
  base, so there is nothing to delete here — only the (already-dangling) links to fix.
- **This is a RE-APPLY, not a merge** of `feat/agent-kit-consume` (different main
  base). Only the self-contained consumption wiring was re-applied; none of the
  reference branch's unrelated commits came along.

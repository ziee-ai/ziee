# Chunk `sdk-agentkit-consume` — DRIFT round 1

**Drift count: 0.**

Definition: a divergence between the plan (CUT + TRANSFORMS) and the realized diff
that isn't a declared, resolved decision.

Checked:

- **Every in-tree skill/lifecycle file in the CUT DELETE table is `git rm`ed AND
  replaced by a committed mode-120000 symlink** — verified via `git ls-files -s`:
  8 `.claude` symlinks (7 skills + `lifecycle`) at `120000`, the 6 lifecycle
  scripts + 7 SKILL.md (+ references) staged `D`, `install-agent-hooks.sh` staged
  `T`. Every symlink `test -e` OK.
- **The submodule is pinned exactly** at `e2e805b` (the reference `c663ef8ea` pin),
  and `sdk` + `pgvector` are untouched — no accidental submodule churn.
- **The consumption glue matches the reference FINAL state** (not intermediate):
  `app.config` byte-identical, `dev-init.sh` is the hardened nullglob-guarded form,
  the justfile recipe block byte-identical (tail diff empty).
- **The `.gitignore` no-slash whitelist + `!.claude/app.config`** exactly matches
  the reference `.gitignore` diff (verified the symlinks + app.config actually
  stage, not ignored).
- **CLAUDE.md** = the reference's @import block + the 7-link repoint + FRONTEND_DEPS
  repoint; DESIGN_SYSTEM.md kept at `./DESIGN_SYSTEM.md`; zero remaining
  `./.claude/[A-Z]*.md` refs (matches reference final).
- **No dangling reference / no STOP-condition** — all 7 in-tree skills + 6 lifecycle
  scripts + the hook installer have agent-kit equivalents; all 8 CLAUDE.md doc
  targets exist under `agent-kit/docs/`.
- **dev-init idempotency** confirmed (git status byte-stable across re-runs) —
  matches the "idempotent" claim in the plan.
- **No code/generated change** — `git status` shows nothing under `.rs` / migrations
  / `Cargo.*` / `package.json` / `openapi.json` / `api-client/types.ts`.

The pre-push-hook install into the shared git-common-dir is a DECLARED decision
(TRANSFORMS D-4 + BOUNDARY), not silent drift.

No unresolved drift → proceed.

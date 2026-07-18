# Chunk `sdk-agentkit-consume` — TRANSFORMS

No symbol/body is *rewritten* here — the skills + lifecycle scripts move OUT of
ziee into the `agent-kit` submodule UNCHANGED (ziee now references the maintained
shared copy through symlinks). The "transforms" are the mechanical consume-model
wiring + the base-reconciliation decisions the re-apply required.

## T-1 — in-tree copies → symlinks (the consume mechanism)

The 7 `.claude/skills/*` dirs + the `.claude/lifecycle/` dir + `scripts/install-agent-hooks.sh`
are DELETED from ziee's tree and replaced by committed symlinks into `agent-kit/`.
The link set is ENUMERATED by `dev-init.sh` from `agent-kit/skills/*` (+ the fixed
`lifecycle` + `install-agent-hooks.sh` targets), so adding/removing a skill in the
submodule can't drift ziee's link set. **why:** one evolving source of truth; ziee
consumes rather than carries the shared agent-dev infra.

## T-2 — `.gitignore` whitelist loses trailing slashes

`!.claude/lifecycle/` → `!.claude/lifecycle` (and the 7 skill entries likewise) +
`+!.claude/app.config`. **why:** a trailing-slash git pattern matches DIRECTORIES
only; a symlink is a FILE, so `!.claude/lifecycle/` would leave the new symlink
IGNORED (untracked). The no-slash form tracks BOTH a real dir (main checkout,
pre-merge) and a symlink (this consume branch). Copied verbatim from the reference.

## T-3 — CLAUDE.md `@import` + doc-link repoint (base-reconciliation)

Added the `@agent-kit/docs/FRAMEWORK.md` @import + consume-model note at the top,
and repointed the 7 framework-doc links from `./.claude/*.md` to `agent-kit/docs/*.md`.
**Decision D-1 (resolved) — apply the FULL reference CLAUDE.md diff, including the
link repoint.** On the `feat/sdk-extraction` base those `./.claude/META_FRAMEWORK_ARCHITECTURE.md`
(etc.) links were ALREADY dangling (the framework docs were never in-tree here —
only `DESIGN_SYSTEM.md` lives at repo root). agent-kit @ `e2e805b` ships all 8 docs
under `docs/`, so the repoint turns 7 dead links into live ones AND matches the
reference model's final state exactly (verified: reference final CLAUDE.md has zero
`./.claude/[A-Z]*.md` refs; FRONTEND_DEPS + FRONTEND framework docs all repointed;
DESIGN_SYSTEM kept at `./DESIGN_SYSTEM.md`).

## T-4 — `scripts/install-agent-hooks.sh` real file → symlink

The 2288-byte in-tree installer is replaced by a symlink to
`../agent-kit/scripts/install-agent-hooks.sh` (git records a type-change `T`).
**why:** single source — the hook installer is shared agent-kit infra like the
skills/lifecycle. `dev-init.sh` step 3 invokes the submodule's installer directly.

## Decision D-2 (resolved) — the initial conversion runs `dev-init.sh` after a manual pre-delete

`dev-init.sh` step 0 is a Windows-symlink SELF-HEAL that ABORTS (`exit 1`) if
`.claude/lifecycle` exists as a real (non-symlink) dir and a `git checkout` can't
re-materialise it as a symlink — correct for a fresh clone of the *already-consumed*
branch (where the symlink is committed), but it would abort the FIRST-TIME
conversion (real dirs still present, nothing committed yet to check out). So the
initial cut removed the in-tree real dirs first, THEN ran `dev-init.sh` — which
sees no real `.claude/lifecycle`, skips step 0, and creates the symlinks via its
enumerate-from-submodule `link()` loop (step 2). This is the exact same end state
the reference committed; `dev-init.sh` remains the drift-proof mechanism for every
subsequent run. Idempotency verified: a 2nd + 3rd `dev-init.sh` run leaves
`git status` byte-stable (step 0 now sees a symlink → skips; `link()` leaves a
correct link as-is).

## Decision D-3 (resolved) — base-reconciliation: agent-kit is the source of truth

Every in-tree skill/lifecycle file on `feat/sdk-extraction` has an identical-in-role
equivalent in agent-kit @ `e2e805b` (7/7 skills, 6/6 lifecycle scripts, the hook
installer). Where content could differ, the symlink points at agent-kit's version
(ziee consumes the maintained shared copy — the intended outcome). No in-tree file
lacked an agent-kit equivalent, so NO dangling symlink was created and the
STOP-and-report condition did not fire.

## Decision D-4 (noted) — the shared pre-push hook install is a real side effect

`dev-init.sh` step 3 writes `$(git rev-parse --git-common-dir)/hooks/pre-push`.
In a worktree that resolves to the COMMON git dir (`/data/pbya/ziee/ziee/.git/hooks`),
shared by every worktree. This is the consume model's intended install, is guarded
(pass-through unless the pushing worktree has `.lifecycle/` or pushes `main`), and
OVERWROTE only the inert `pre-push.sample` (no prior custom hook clobbered). No push
is performed. Recorded here + in BOUNDARY so the human is aware the shared hook now
exists.

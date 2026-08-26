# DECISIONS — upstream-port

### DEC-1: How is `khoi` brought up to date, and what is the conflict policy?
**Resolution:** `git worktree add -b khoi … upstream/khoi` then `git merge --ff-only
upstream/main`. It fast-forwarded 43 files with zero conflicts, so the brief's
"prioritise upstream/main on any conflict" rule never had to fire.
**Basis:** user — the brief's §"How (A) lands" steps 1-2. Verified the precondition
first: `git branch -r --contains d65308170` lists both `upstream/khoi` and
`upstream/main`, so `khoi` was strictly behind and had nothing to lose.

### DEC-2: Which paws changes go up?
**Resolution:** Ten defects (ITEM-1..ITEM-9 plus the hygiene assertion), each verified
to be STILL PRESENT in `upstream/main` before being ported. Everything in the brief's
four permanently-excluded buckets is out, and so are paws' CI, updater
endpoint/pubkey, and desktop README.
**Basis:** user — the owner picked "All 10 (minus macOS-unverifiable)" from an
explicit option picker that also offered a Tier-1-only narrow diff and an
all-11-including-ggml variant.

### DEC-3: Port by cherry-pick or by hand?
**Resolution:** **By hand, against upstream's shape.** Not one paws commit is
replayed. Several mix in-scope and out-of-scope hunks in the same file — the clearest
case is `uploads.rs`, where paws' diff contains BOTH the LFS forwarder (in scope) and
a `repository.git_credential()` extraction that depends on a `models.rs` method paws
added for its default-model work (out of scope). A blind `git checkout` of that file
would have compiled nowhere and dragged paws product code upstream.
**Basis:** user — the brief: "Do not cherry-pick blindly — several of these commits
mix in-scope and out-of-scope changes. Port the behaviour, not the commit."

### DEC-4: The GPU/CUDA fix — port, rewrite, or escalate?
**Resolution:** **Escalate; port nothing.** Upstream still has the defect twice over
(its `gpu_detect.rs` scrapes the literal `CUDA Version:` 4×; its pinned sdk's
`ziee-hardware/src/detection.rs:201` has the same scrape). But paws' fix moved the
parser into a NEW sdk module, `crates/ziee-hardware/src/gpu_version.rs`, which does
not exist at sdk `4ab75300`; paws' `gpu_detect.rs:37` is
`use ziee_hardware::gpu_version::{self, MajorMinor};`, so the superproject half does
not compile upstream. Writing a divergent inline fix was considered and rejected: it
would conflict with the real sdk change when that lands.
**Basis:** user — the brief is explicit ("if a port needs an sdk change, STOP and
escalate — the sdk is shared with another product line and the branch choice is the
owner's"), and the owner separately confirmed the sdk work should go to `chat` as its
own PR.

### DEC-5: The CORS fix has an sdk half and a superproject half. Take which?
**Resolution:** The superproject half only. paws' `create_cors_layer` shim calls
`ziee_framework::app_builder::create_cors_layer_with` + `FRAMEWORK_REQUIRED_REQUEST_HEADERS`,
neither of which exists at sdk `4ab75300` (it has only `create_cors_layer`, at
`app_builder.rs:201`). ITEM-2 adds the header to the desktop's explicit allowlist and
both shipped example configs, which closes the actual user-visible bug with no sdk
dependency. The union — which makes omission *inexpressible* — goes with the sdk PR.
**Basis:** codebase — verified by listing the sdk blob at that commit.

### DEC-6: ITEM-3 ports a fix from a commit that is otherwise excluded. Is that allowed?
**Resolution:** Yes, and the split is the whole point. Paws' `816aa6321` contains two
different things: the kill-switch route GUARD (shared-code security — with the switch
off, `js_tool`'s JSON-RPC endpoint stayed mounted and gated only on `js_tool::use`,
which migration `202607146040` grants to the Users group, so any ordinary user could
still execute arbitrary QuickJS) and paws' DEFAULT FLIPS (`js_tool_enabled()` etc.,
pure product direction). The guard is ported; the flips are not, and TEST-5 —
upstream's own `config_default_enabled`, left untouched — is the control that proves
it.
**Basis:** convention — the brief's rule is "anything hiding/reducing features" is
paws product. A default flip hides a feature; a guard that makes `disabled` mean
"not served" is a security fix.

### DEC-7: Does `voice` get changed too?
**Resolution:** Only its fail-closed default (`enabled: true` → `false` in `new()`).
Upstream's `VoiceModule` already guards `register_routes` correctly, so it needs
nothing else; but its `new()` still defaulted to the ENABLED value, which is the
"stale initializer defaulting a kill switch to ON" shape INV-3 names. Its paws-side
default flip is NOT ported.
**Basis:** codebase — read upstream's `voice/mod.rs` and diffed paws' against it to
separate the two changes.

### DEC-8: `tauri.conf.json` contains an in-scope hunk and an excluded one.
**Resolution:** Port ONLY `beforeBuildCommand`. The sibling `updater` hunk in the same
file swaps the endpoint to `tinnlab.github.io/paws` and the minisign pubkey to paws' —
porting that would point every upstream desktop build's auto-updater at paws'
release feed.
**Basis:** user — the brief's hard exclusion list.

### DEC-9: ITEM-11 is not a port at all. Include it?
**Resolution:** Yes, as its own commit. While verifying the port I ran two unit tests
in this worktree (upstream/main + this branch's changes, neither of which touches
either file) and both FAIL — so `ziee-ai/ziee` `main` is red today. Upstream has no PR
CI, which is how they survived. Both fixes are small and were verified RED-then-GREEN.
Kept as a separate commit so the owner can drop it independently of the ports.
**Basis:** convention — leaving a repo's own suite red when the fix is known and
five lines would be the worse call; isolating it in one commit keeps the port
reviewable.

### DEC-10: Commit authorship.
**Resolution:** Every commit is authored AND committed by `khoi
<khoi@tinnguyen-lab.com>`. Unlike the pull-down branch — where clean cherry-picks
preserve the upstream author — nothing here is a cherry-pick: every hunk was
hand-written against upstream's shape, so `khoi` is the actual author. Each commit
body names the paws commit whose behaviour it realizes.
**Basis:** user — the brief's "commit and push as khoi", plus honest attribution.
**No Claude/AI attribution anywhere**, per the brief.

### DEC-11: Where do the lifecycle artifacts go?
**Resolution:** Stripped in a final `chore:` commit before the PR is opened, leaving
them in branch history.
**Basis:** codebase — upstream's own precedent (`a23726215`, `db2347928`,
`0766eff12`, all titled "chore: strip lifecycle artifacts before merge"). Since the
owner performs the merge and this worker must not, the strip happens on the branch.

### DEC-12: PR target and merge authority.
**Resolution:** PR `khoi` → `ziee-ai/ziee` `main`. **Opened, never merged.** Nothing
pushed to any `main`. No submodule pointer moved in this PR — asserted by ITEM-10.
**Basis:** user — the brief's standing rules.

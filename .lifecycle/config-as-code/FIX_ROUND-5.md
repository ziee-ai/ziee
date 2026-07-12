# FIX_ROUND-5 — fifth blind round

The round-5 agent re-verified the rename (manifest ⇄ reconciler ⇄ unit test ⇄ integration suite ⇄
README all agree on `*-user`; no double-suffixing; the bare `rcpa`/`dscc` strings that remain are
self-contained PARSER fixtures), the `extra_hosts` placement (YAML-parsed both compose files), and
that `splitPatterns` + the new SQL is now exactly equivalent to Rust `permission_matches`.

Four LOW findings, all fixed:

1. **A generated artifact was accidentally committed.** My `gate:ui` run regenerated
   `src/dev/gallery/RUNTIME_FINDINGS.{md,jsonl}` and `git add -A` swept it in — even though this diff
   touches ZERO files under `src-app/ui/src`. The committed run was machine-local noise that made the
   baseline look WORSE than base (crash 4 → 8), which would have sent the next person chasing phantom
   regressions. Reverted to base.
2. **A negative control went vacuous.** `mcp-user-servers.spec.ts`'s search test asserted that the
   `Filesystem` card is absent after searching "Web Fetch" — but migration 157 deleted that row, so
   the assertion would now pass even if search were a no-op. The spec now creates its own
   `Search Probe` server and asserts THAT is filtered out. (This is the 4th spec the deletion touched;
   the audit caught the one I'd missed.) Re-ran: 15/15 pass.
3. **The rename/orphan caveat was undocumented — and is real.** A server's identity is its `name`, and
   the manifest only ever adds/updates; it never deletes. So renaming an entry (exactly what the
   `*-user` change did) leaves the OLD row behind, still enabled and still group-assigned. Now stated
   explicitly in both `config/desired-state.yaml`'s RULES block and the README, with the remedy
   (delete the stale server once in Settings → MCP Servers). Not a problem for a fresh deploy, but a
   trap worth naming.
4. `config-as-code.STATUS.md` still used the old row names. Updated.

**New confirmed findings:** 4

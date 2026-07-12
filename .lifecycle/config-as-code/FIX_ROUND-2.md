# FIX_ROUND-2 — re-audit of the fixed diff

Two fresh blind agents re-audited the post-fix diff, targeting what the FIXES themselves broke.

**Verified clean by the re-audit:** the `assign_to_group` / `remove_from_group` parameter rename is a
true no-op — all five pre-existing call sites (`mcp/handlers/groups.rs` ×3, `desktop/tauri/.../mcp/event_handlers.rs` ×2)
already passed `(group, server)` positionally, so only the names changed. The advisory-lock key is in
range, no `?` can leak the lock, `is_legal_add` is airtight against the checker's wildcard semantics,
no `tracing::` call or error string interpolates a secret, and the default `docker compose up` (no env
set) does NOT crash-loop.

## Confirmed and fixed

1. **The debug seam did not cover the code path that needed it (HIGH — caught by the tests failing).**
   `ZIEE_DISABLE_MCP_HEALTH_CHECK` was honored by `enforce_on_create`/`enforce_on_update` but NOT by
   `run_startup_health_check` — the boot sweep. My integration tests set the var and still had their
   servers auto-disabled underneath the assertions (3 tests failed, exactly as the auditors predicted).
   The sweep now honors the seam its own name promises (debug-gated; production is unchanged).
2. **The advisory lock could be stranded for the process lifetime (MED).** `drop(conn)` returns a LIVE
   session to the sqlx pool, so a failed `pg_advisory_unlock` would leave a session-level lock held —
   blocking every peer container's boot. On unlock failure the connection is now CLOSED, which makes
   Postgres drop the lock with the session. The lock acquire is also bounded (30s) rather than an
   unbounded `pg_advisory_lock`.
3. **An `enforce` re-sync failure skipped group convergence (LOW)** — violating this module's own
   "group availability always converges" contract. It now logs and carries on to the assignment.
4. **Vacuous re-deploy assertions (MED).** TEST-8/TEST-9 asserted only the ABSENCE of change, so they
   would have stayed green if the second process had reconciled nothing at all. Each now deletes a
   declared row first and asserts it comes back — a positive control that the second boot really ran.
5. **Doc honesty (LOW).** The module claimed the fatal-on-bad-file rule covered "no admin"; it does
   not — an unset `ZIEE_ADMIN_PASSWORD` is a deliberate soft-skip that leaves ziee's ORDINARY
   fresh-install state (open first-run setup page), which this module does not introduce and which
   making fatal would break the documented quick-start. The warning now says so explicitly. The
   `add:` wildcard guard is likewise documented as a footgun guard, not a trust boundary.
6. The e2e manifest parser now stops at the end of the contiguous `remove:` list.

**New confirmed findings:** 6

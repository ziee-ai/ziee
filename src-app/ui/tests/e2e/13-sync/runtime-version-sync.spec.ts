import { test } from '../../fixtures/test-context'

// Realtime sync for the `runtime_version` entity (admin local-engine
// versions table) is currently E2E-DEFERRED.
//
// Why this is empty (intentional, not forgotten):
//
//   The three handlers that publish `RuntimeVersion/*`
//   (delete_version, set_system_default, sync_cache) all need a
//   pre-existing runtime version row to mutate, and there is no
//   E2E-reachable way to seed one without either:
//
//     1. A real download via POST /api/local-runtime/versions/download
//        — fetches binaries from GitHub Releases (not test-friendly
//        without a mock release server). The
//        `ZIEE_E2E_ENGINE_MIRROR` env-var path the
//        `12-local-runtime/04-engine-lifecycle.spec.ts` uses requires
//        an operator to point at a real mirror; the test suite skips
//        those flows when unset.
//     2. Direct DB insert — outside the E2E test boundary.
//     3. `sync_cache` over a pre-populated cache directory — would
//        need a fixture that stages binary files into the backend's
//        `llm_engines_dir` BEFORE the test, which the per-worker test
//        infrastructure doesn't expose today.
//
//   The Tier-2 integration suite already proves the sync emit on the
//   real HTTP path:
//     - server/tests/llm_local_runtime/sync_emit_test.rs::set_default_delivers_runtime_version_update_other_user_silent
//     - server/tests/llm_local_runtime/sync_emit_test.rs::delete_delivers_runtime_version_delete_other_user_silent
//   (those tests use `seed_version` to insert directly via PgPool —
//   the same pattern that's unavailable from a browser context.)
//
//   The UI subscriber path
//   (`RuntimeVersion.store.ts::eventBus.on('sync:runtime_version', ...)`)
//   is identical in shape to the RuntimeSettings subscriber that IS
//   browser-tested by `admin-settings-sync.spec.ts`, so the
//   reload-on-event chain is exercised end-to-end via the sibling
//   entity.
//
// What WOULD be tested if the infrastructure existed:
//
//   Two admin browser contexts (device A + B) on
//   /settings/llm-runtime. A POSTs `/api/local-runtime/versions/{id}/set-default`,
//   B's version table reflects the new system-default flag without
//   reload. Then A DELETEs the version; B's row disappears.
//
// Tracking: revisit when `tests/fixtures/test-context.ts` grows a
// pre-seed hook for engine binaries (a Node port of
// `seed_version` + filesystem staging), OR when the engine-mirror
// E2E suite (12-local-runtime/04-engine-lifecycle.spec.ts) lands a
// shared seed helper that this spec can reuse.

test.describe.skip('Realtime sync — runtime_version (deferred — needs engine-seed fixture)', () => {
  test('placeholder', () => {})
})

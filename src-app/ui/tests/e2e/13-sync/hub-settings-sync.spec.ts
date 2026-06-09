import { test } from '../../fixtures/test-context'

// Realtime sync for the `hub_settings` singleton entity is currently
// E2E-DEFERRED.
//
// Why this is empty (intentional, not forgotten):
//
//   The two handlers that publish `HubSettings/Update`
//   (POST /api/hub/refresh and POST /api/hub/activate) both call
//   `HubManager::refresh()` FIRST, which fetches + cosign-verifies a
//   catalog bundle from GitHub. In tests this can only succeed when
//   the backend is pointed at a mock hub via
//   `ZIEE_HUB_API_BASE_OVERRIDE` / `ZIEE_HUB_DOWNLOAD_BASE_OVERRIDE` /
//   `ZIEE_HUB_ALLOW_UNSIGNED=1` AND a loopback HTTP server is serving
//   a tiny tar.gz catalog (the `spawn_mock_hub` fixture used by
//   `server/tests/hub/sync_emit_test.rs` does exactly this).
//
//   Wiring this for E2E requires:
//     1. A Playwright fixture that spawns the mock hub HTTP server
//        (a Node port of `mock_release_server.rs`) before the test.
//     2. A per-test env override path in `tests/fixtures/test-context.ts`
//        so the backend is spawned with the mock URL in its env. Today
//        the backend env is fixed per worker.
//
//   That's a non-trivial infrastructure addition (probably ~half a
//   day of plumbing) and the underlying sync emit is already proven
//   end-to-end by `server/tests/hub/sync_emit_test.rs::activate_delivers_hub_settings_update_other_user_silent`.
//   The UI subscriber path is identical in shape to the LlmRepository
//   subscriber that IS browser-tested in `admin-settings-sync.spec.ts`.
//
// What WOULD be tested if the infrastructure existed:
//
//   Two admin browser contexts (device A + B) on `/hub`. A POSTs
//   `/api/hub/activate` (pinning a different bundled mock version),
//   B's HubPage header version label and tab counts refresh without
//   a manual reload — proves the `sync:hub_settings` listener on
//   `useHubCatalogStore` reloads all tabs.
//
// Tracking: revisit when test-context grows a per-test env-injection
// API (e.g. `testInfra.backendEnv(record)`), or when the hub gains a
// settings mutation that doesn't depend on `HubManager::refresh()`.

test.describe.skip('Realtime sync — hub_settings (deferred — needs mock-hub fixture)', () => {
  test('placeholder', () => {})
})

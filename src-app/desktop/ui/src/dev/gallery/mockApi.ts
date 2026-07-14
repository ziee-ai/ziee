/**
 * ziee-desktop's binding of the generic `@ziee/gallery` mock-API engine to this
 * workspace's generated api-client. The ENGINE (route matching, state modes, SSE
 * replay, safe-empty proxy) lives in `@ziee/gallery`; this shim binds `Cassette`
 * to the DESKTOP `ApiEndpointResponses` (so every desktop cassette is `tsc`-checked)
 * and re-exports the runtime for the fixtures + boot. Mirrors the web workspace's
 * `mockApi.ts` shim.
 */
import type { ApiEndpointResponses } from '@/api-client/types'
import type {
  Cassette as GCassette,
  CassetteEntry as GCassetteEntry,
} from '@ziee/gallery'

/** ziee-bound cassette: typed against the desktop generated api-client response map. */
export type Cassette = GCassette<ApiEndpointResponses>
export type CassetteEntry<K extends keyof ApiEndpointResponses> =
  GCassetteEntry<ApiEndpointResponses[K]>

export type {
  MockRequestContext,
  SseFrame,
  SpecialRoute,
  MockMode,
} from '@ziee/gallery'

export {
  configureMockApi,
  installMockApi,
  uninstallMockApi,
  setCassette,
  extendCassette,
  setMockMode,
  getMockMode,
  setSseCassette,
  sseResponse,
  sseReplayResponse,
  jsonResponse,
  mockErrorResponse,
  makeBinaryResponse,
  base64ToBytes,
} from '@ziee/gallery'

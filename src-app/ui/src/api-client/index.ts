import {
  ApiEndpoint,
  ApiEndpointParameters,
  ApiEndpointResponses,
  ApiEndpoints,
} from '@/api-client/types'
import {
  createApiClient,
  getAuthToken,
  setBaseUrlResolver,
} from '@ziee/framework/api-client'
import type {
  FileUploadProgressCallback,
  SSECallback,
} from '@ziee/framework/api-client'
import { getBaseUrl } from '@/api-client/getBaseURL'

// Per-app binding of the framework ApiClient runtime.
//
// The transport (`createApiClient`, `callAsync`, auth/refresh/retry/SSE) moved
// into `@ziee/framework/api-client`; the generated per-endpoint types
// (`ApiEndpoints`, `ApiEndpointParameters`, `ApiEndpointResponses`) stay
// per-app OUTPUT in this app's `api-client/types.ts`. This file is the thin
// binding layer that supplies those concrete types to the generic factory —
// the runtime is identical to before, only the type-map is app-owned.

// Register this app's base-URL resolver with the framework transport. The
// resolver is `@/api-client/getBaseURL`, which the desktop build swaps for the
// Tauri dynamic-port variant via its `.desktop` override — so the framework
// stays platform-agnostic while the desktop base-URL behavior is preserved.
setBaseUrlResolver(getBaseUrl)

// Re-export utility functions
export { getAuthToken }

// Helper types for automatic namespace/method extraction
type ExtractNamespace<T extends string> = T extends `${infer N}.${string}`
  ? N
  : never
type Namespaces = ExtractNamespace<ApiEndpoint>

// Force TypeScript to evaluate and display concrete types
type Evaluate<T> = T extends infer U ? { [K in keyof U]: U[K] } : never

// Force resolution of parameter types using identity function
type ResolveParams<K extends ApiEndpoint> =
  K extends keyof ApiEndpointParameters ? ApiEndpointParameters[K] : never

// Force resolution of response types
type ResolveResponse<K extends ApiEndpoint> =
  K extends keyof ApiEndpointResponses ? ApiEndpointResponses[K] : never

// Dynamic namespace methods with concrete type resolution
type NamespaceMethods<N extends Namespaces> = Evaluate<{
  [K in ApiEndpoint as K extends `${N}.${infer M}` ? M : never]: (
    params: ResolveParams<K>,
    callbacks?: {
      SSE?: SSECallback<ResolveResponse<K>>
      fileUploadProgress?: FileUploadProgressCallback
    },
  ) => Promise<ResolveResponse<K>>
}>

// Main ApiClient type - fully dynamic, scalable, and with resolved types
type ApiClientType = Evaluate<{
  [N in Namespaces]: NamespaceMethods<N>
}>

// Export the main ApiClient — the generic framework factory instantiated with
// this app's generated `ApiEndpoints` map and cast to the app-typed shape.
export const ApiClient = createApiClient<ApiClientType>(ApiEndpoints)

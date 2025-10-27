import {
  ApiEndpoint,
  ApiEndpointParameters,
  ApiEndpointResponses,
  ApiEndpoints,
} from './types'
import { callAsync, FileUploadProgressCallback } from './core'
import type { SSECallback, SSEEventKey, SSEEventData } from './sse-types'

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

function createApiClient(): ApiClientType {
  const client = {} as any

  // Get all endpoint keys and group by namespace
  const endpointKeys = Object.keys(ApiEndpoints) as ApiEndpoint[]

  endpointKeys.forEach(endpointKey => {
    const [namespace, method] = endpointKey.split('.') as [string, string]

    if (!client[namespace]) {
      client[namespace] = {}
    }

    // Create the method that calls callAsync with proper typing
    client[namespace][method] = async (
      params: any,
      callbacks?: {
        SSE?: <E extends SSEEventKey<any>>(
          event: E,
          data: SSEEventData<any, E>,
        ) => void
        fileUploadProgress?: FileUploadProgressCallback
      },
    ) => {
      return callAsync(ApiEndpoints[endpointKey], params, callbacks as any)
    }
  })

  return client as ApiClientType
}

// Export the main ApiClient - dynamically generated, fully type-safe
export const ApiClient = createApiClient()

import { ApiClient } from '@/api-client'
import { type SandboxFlavorsResponse } from '@/api-client/types'
import type { McpServerSet } from '../state'

/**
 * Lazily fetched by the system-server form to populate the sandbox flavor
 * picker. Admin-gated; only called from create-system/edit-system mode.
 */
export default (_set: McpServerSet, _get: () => never) =>
  async (): Promise<SandboxFlavorsResponse> =>
    await ApiClient.CodeSandbox.listFlavors()

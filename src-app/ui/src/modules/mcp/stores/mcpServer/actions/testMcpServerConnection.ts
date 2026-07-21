import { ApiClient } from '@/api-client'
import { type TestMcpConnectionRequest, type TestMcpConnectionResponse } from '@/api-client/types'
import type { McpServerSet } from '../state'

/** Probe a candidate config (read-only; nothing persisted). 200 even on failure. */
export default (_set: McpServerSet, _get: () => never) =>
  async (data: TestMcpConnectionRequest): Promise<TestMcpConnectionResponse> =>
    await ApiClient.McpServer.testConnection(data)

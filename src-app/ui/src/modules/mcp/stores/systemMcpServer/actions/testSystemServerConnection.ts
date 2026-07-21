import { ApiClient } from '@/api-client'
import { type TestMcpConnectionRequest, type TestMcpConnectionResponse } from '@/api-client/types'
import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

// Probe a candidate config (read-only; nothing persisted). 200 even on failure.
export default (_set: SystemMcpServerSet, _get: SystemMcpServerGet) =>
  async (data: TestMcpConnectionRequest): Promise<TestMcpConnectionResponse> =>
    await ApiClient.McpServerSystem.testConnection(data)

import type { StoreSet } from '@ziee/framework/store-kit'
import type { HubMCPServer, McpServer } from '@/api-client/types'

export const mcpServersStepState = {
  selectedMcpServerIds: [] as string[],
  systemServers: [] as McpServer[],
  hubServers: [] as HubMCPServer[],
  installedNames: new Set<string>(),
  /** IDs of system servers the user wants DISABLED */
  disabledSystemIds: new Set<string>(),
  /** Snapshot of disabledSystemIds at load — used to compute the diff on apply */
  originalDisabledSystemIds: new Set<string>(),
  loadingServers: false,
  serversError: null as string | null,
}

export type McpServersStepState = typeof mcpServersStepState
export type McpServersStepSet = StoreSet<McpServersStepState>
export type McpServersStepGet = () => McpServersStepState

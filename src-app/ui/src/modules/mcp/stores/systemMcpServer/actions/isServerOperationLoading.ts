import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (_set: SystemMcpServerSet, get: SystemMcpServerGet) =>
  (serverId: string, operation?: string): boolean => {
    const { operationsLoading } = get()
    const operationKey = operation ? `${serverId}-${operation}` : serverId
    return operationsLoading.get(operationKey) || false
  }

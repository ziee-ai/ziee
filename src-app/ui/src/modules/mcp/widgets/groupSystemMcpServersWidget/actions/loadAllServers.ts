import type { GroupSystemMcpServersWidgetSet, GroupSystemMcpServersWidgetGet } from '../state'
import loadAllServersFactory from './_loadAllServers'

export default (set: GroupSystemMcpServersWidgetSet, get: GroupSystemMcpServersWidgetGet) => {
  const loadAllServers = loadAllServersFactory(set, get)
  return async (): Promise<void> => loadAllServers()
}

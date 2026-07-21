import type { GroupSystemMcpServersWidgetSet, GroupSystemMcpServersWidgetGet } from '../state'

export default (_set: GroupSystemMcpServersWidgetSet, get: GroupSystemMcpServersWidgetGet) =>
  (groupId: string) => {
    return get().groupServers.get(groupId)
  }

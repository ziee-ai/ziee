import type { GroupSystemMcpServersWidgetSet, GroupSystemMcpServersWidgetGet } from '../state'

export default (set: GroupSystemMcpServersWidgetSet, _get: GroupSystemMcpServersWidgetGet) =>
  (groupId: string): void => {
    set(s => {
      s.groupServers.delete(groupId)
    })
  }

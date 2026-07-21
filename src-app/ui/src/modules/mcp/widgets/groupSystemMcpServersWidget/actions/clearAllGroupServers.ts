import type { GroupSystemMcpServersWidgetSet, GroupSystemMcpServersWidgetGet } from '../state'

export default (set: GroupSystemMcpServersWidgetSet, _get: GroupSystemMcpServersWidgetGet) =>
  (): void => {
    set(s => {
      s.groupServers.clear()
    })
  }

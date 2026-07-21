import type { SystemMcpServerGet, SystemMcpServerSet } from '../state'

export default (set: SystemMcpServerSet, _get: SystemMcpServerGet) => () => {
  set({ systemServersError: null })
}

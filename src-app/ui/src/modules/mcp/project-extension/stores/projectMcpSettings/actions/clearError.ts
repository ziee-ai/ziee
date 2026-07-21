import type { ProjectMcpSettingsSet } from '../state'

export default (set: ProjectMcpSettingsSet) => () => {
  set(s => {
    s.error = null
  })
}

import type { ScheduledTasksGet, ScheduledTasksSet } from '../state'

export default (set: ScheduledTasksSet, _get: ScheduledTasksGet) => {
  return () => {
    set(draft => {
      draft.error = null
    })
  }
}

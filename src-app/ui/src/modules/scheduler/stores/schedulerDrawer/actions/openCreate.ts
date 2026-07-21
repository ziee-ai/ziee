import type { SchedulerDrawerSet } from '../state'

export default (set: SchedulerDrawerSet) => () =>
  set(s => {
    s.open = true
    s.editing = null
  })

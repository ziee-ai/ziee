import type { SchedulerDrawerSet } from '../state'

export default (set: SchedulerDrawerSet) => () =>
  set(s => {
    s.open = false
    s.editing = null
    s.loading = false
  })

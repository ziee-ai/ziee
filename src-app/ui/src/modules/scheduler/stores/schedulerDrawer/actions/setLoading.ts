import type { SchedulerDrawerSet } from '../state'

export default (set: SchedulerDrawerSet) => (loading: boolean) =>
  set(s => {
    s.loading = loading
  })

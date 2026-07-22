import { setOverlayOpen } from '@/core/overlays/overlayVisibility'
import type { ScheduledTask } from '@/api-client/types'
import type { SchedulerDrawerSet } from '../state'

export default (set: SchedulerDrawerSet) => (_task: ScheduledTask) =>
  set(s => {
    s.open = true
    setOverlayOpen('scheduler', true)
    s.editing = _task
  })

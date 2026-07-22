import { setOverlayOpen } from '@/core/overlays/overlayVisibility'
import type { SchedulerDrawerSet } from '../state'

export default (set: SchedulerDrawerSet) => () =>
  set(s => {
    s.open = true
    setOverlayOpen('scheduler', true)
    s.editing = null
  })

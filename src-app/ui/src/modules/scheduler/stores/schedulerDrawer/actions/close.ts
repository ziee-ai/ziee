import { setOverlayOpen } from '@/core/overlays/overlayVisibility'
import type { SchedulerDrawerSet } from '../state'

export default (set: SchedulerDrawerSet) => () =>
  set(s => {
    s.open = false
    setOverlayOpen('scheduler', false)
    s.editing = null
    s.loading = false
  })

import type { UpdaterSet } from '../state'

export default (set: UpdaterSet, _get: () => unknown) => () => {
  // Hide the card this session; it reappears on the next launch's check.
  set(s => {
    s.dismissed = true
  })
}

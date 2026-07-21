import type { SandboxRootfsVersionsSet } from '../state'
import { cleanupSseState } from '../_sse'

export default (_set: SandboxRootfsVersionsSet) => {
  return () => {
    cleanupSseState()
  }
}

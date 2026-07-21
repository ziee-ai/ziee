import { Stores } from '@ziee/framework/stores'
import type { VoiceModelGet, VoiceModelSet } from '../state'

export default (_set: VoiceModelSet, _get: VoiceModelGet) =>
  async () => {
    await Stores.VoiceModelUpdate.checkForUpdates().catch(() => {
      /* non-fatal */
    })
  }

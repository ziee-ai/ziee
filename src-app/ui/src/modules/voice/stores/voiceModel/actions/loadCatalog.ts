import type { VoiceModelGet, VoiceModelSet } from '../state'
import { VoiceModelUpdate } from '@/modules/voice/stores/voiceModelUpdate'

export default (_set: VoiceModelSet, _get: VoiceModelGet) =>
  async () => {
    await VoiceModelUpdate.checkForUpdates().catch(() => {
      /* non-fatal */
    })
  }

import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { createVoiceStore } from './Voice.store'
import { MicButton } from './components/MicButton'

/**
 * Voice Extension
 *
 * Adds a microphone button to the composer toolbar for local voice dictation.
 * Recording is captured via MediaRecorder, converted to 16 kHz mono WAV, and
 * POSTed to `/voice/transcribe`; the returned text is APPENDED to the composer
 * (never auto-sent). All state lives in VoiceStore (`Stores.Chat.VoiceStore`),
 * whose `init` fetches the readiness capability so the button can hide/disable
 * itself appropriately.
 */
const voiceExtension: ChatExtension = createExtension({
  name: 'voice',
  description: 'Local voice dictation into the chat composer',
  priority: 85,

  store: {
    name: 'VoiceStore',
    createStore: createVoiceStore,
  },

  // Sits just left of the keyboard-tips text (order 90) in the toolbar.
  slots: {
    toolbar_actions: { component: MicButton, order: 85 },
  },
})

export default voiceExtension

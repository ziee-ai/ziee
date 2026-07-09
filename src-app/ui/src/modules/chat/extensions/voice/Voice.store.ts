import { message } from '@/components/ui'
import { ApiClient } from '@/api-client'
import { Permissions, type VoiceCapability } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { defineExtensionStore } from '@/modules/chat/core/extensions'
import { recordedBlobToWav16k } from './audio/wav'

/**
 * VoiceStore — the chat-composer voice-dictation state machine.
 *
 * status flow:
 *   idle → requesting (getUserMedia prompt) → recording (MediaRecorder running)
 *        → transcribing (WAV encode + POST /voice/transcribe) → idle
 *   any failure → error (auto-reverts to idle after a short delay)
 *
 * Non-serializable resources (MediaRecorder / MediaStream / timers) live in
 * module scope — never in immer state (immer would freeze them).
 *
 * Read as `Stores.Chat.VoiceStore`; actions are callable directly, and
 * handler-side state reads use the `$` snapshot.
 */

export type VoiceStatus =
  | 'idle'
  | 'requesting'
  | 'recording'
  | 'transcribing'
  | 'error'

// ── Module-scope imperative resources (not reactive / not serializable) ──────
let mediaRecorder: MediaRecorder | null = null
let mediaStream: MediaStream | null = null
let chunks: Blob[] = []
let elapsedTimer: ReturnType<typeof setInterval> | null = null
let stageTimer: ReturnType<typeof setTimeout> | null = null
let errorRevertTimer: ReturnType<typeof setTimeout> | null = null
let recordStartedAt = 0

function stopStream(): void {
  if (mediaStream) {
    for (const track of mediaStream.getTracks()) track.stop()
    mediaStream = null
  }
  mediaRecorder = null
}

function clearTimers(): void {
  if (elapsedTimer !== null) {
    clearInterval(elapsedTimer)
    elapsedTimer = null
  }
  if (stageTimer !== null) {
    clearTimeout(stageTimer)
    stageTimer = null
  }
}

/**
 * True when the browser can actually capture a mic: a secure context (https or
 * localhost) with a `mediaDevices.getUserMedia`. Used by the button to HIDE
 * itself where recording is impossible (matches the capability-disabled hide).
 */
export function isRecordingSupported(): boolean {
  return (
    typeof window !== 'undefined' &&
    (window.isSecureContext ?? false) &&
    typeof navigator !== 'undefined' &&
    !!navigator.mediaDevices &&
    typeof navigator.mediaDevices.getUserMedia === 'function' &&
    typeof window.MediaRecorder !== 'undefined'
  )
}

export const createVoiceStore = defineExtensionStore({
  immer: false,
  state: {
    status: 'idle' as VoiceStatus,
    /** Elapsed record time in ms (drives the on-screen timer). */
    elapsedMs: 0,
    /** Readiness snapshot; null until fetched. */
    capability: null as VoiceCapability | null,
    capabilityLoaded: false,
    /** Staged status line shown while transcribing (cold-start aware). */
    stageText: '',
    /** Last error message (for the aria-live region). */
    errorMessage: null as string | null,
  },
  actions: (set, get) => {
    const revertToIdleSoon = () => {
      if (errorRevertTimer !== null) clearTimeout(errorRevertTimer)
      errorRevertTimer = setTimeout(() => {
        errorRevertTimer = null
        if (get().status === 'error') {
          set({ status: 'idle', errorMessage: null, elapsedMs: 0, stageText: '' })
        }
      }, 2500)
    }

    const fail = (msg: string) => {
      clearTimers()
      stopStream()
      chunks = []
      message.error(msg)
      set({ status: 'error', errorMessage: msg, elapsedMs: 0, stageText: '' })
      revertToIdleSoon()
    }

    /** Fetch the readiness snapshot. Self-gates on the transcribe permission. */
    const fetchCapability = async () => {
      if (!hasPermissionNow(Permissions.VoiceTranscribe)) {
        set({ capabilityLoaded: true })
        return
      }
      try {
        const capability = await ApiClient.Voice.capability()
        set({ capability, capabilityLoaded: true })
      } catch {
        // Non-fatal: the button hides when capability stays null.
        set({ capabilityLoaded: true })
      }
    }

    const startRecording = async () => {
        const { status, capability } = get()
        if (status !== 'idle' && status !== 'error') return
        if (!isRecordingSupported()) {
          fail('Voice recording is not supported in this browser.')
          return
        }
        set({ status: 'requesting', errorMessage: null, elapsedMs: 0, stageText: '' })
        let stream: MediaStream
        try {
          stream = await navigator.mediaDevices.getUserMedia({ audio: true })
        } catch (err) {
          const denied =
            err instanceof DOMException &&
            (err.name === 'NotAllowedError' || err.name === 'SecurityError')
          fail(
            denied
              ? 'Microphone access was denied. Allow it in your browser to dictate.'
              : 'Could not start recording — no microphone available.',
          )
          return
        }
        mediaStream = stream
        chunks = []
        try {
          const recorder = new MediaRecorder(stream)
          mediaRecorder = recorder
          recorder.ondataavailable = e => {
            if (e.data && e.data.size > 0) chunks.push(e.data)
          }
          recorder.start()
        } catch {
          fail('Could not start the audio recorder.')
          return
        }
        recordStartedAt = Date.now()
        set({ status: 'recording', elapsedMs: 0 })
        const maxMs = (capability?.max_clip_seconds ?? 60) * 1000
        elapsedTimer = setInterval(() => {
          const elapsed = Date.now() - recordStartedAt
          set({ elapsedMs: elapsed })
          if (elapsed >= maxMs) {
            void stopRecording()
          }
        }, 200)
    }

    const stopRecording = async () => {
        if (get().status !== 'recording' || !mediaRecorder) return
        clearTimers()
        const recorder = mediaRecorder
        // Await the assembled blob via onstop before we tear the stream down.
        const recorded = await new Promise<Blob>(resolve => {
          recorder.onstop = () => {
            resolve(new Blob(chunks, { type: recorder.mimeType || 'audio/webm' }))
          }
          try {
            recorder.stop()
          } catch {
            resolve(new Blob(chunks, { type: 'audio/webm' }))
          }
        })
        stopStream()
        chunks = []

        if (recorded.size === 0) {
          fail('No audio was captured — try recording again.')
          return
        }

        set({ status: 'transcribing', stageText: 'Starting voice engine…' })
        // Cold-start staging: whisper-server may autostart on the first clip, so
        // start with "Starting…" and flip to "Transcribing…" if it lingers.
        stageTimer = setTimeout(() => {
          if (get().status === 'transcribing') set({ stageText: 'Transcribing…' })
        }, 1200)

        try {
          const wav = await recordedBlobToWav16k(recorded)
          const formData = new FormData()
          formData.append('file', wav, 'dictation.wav')
          const result = await ApiClient.Voice.transcribe(formData)
          clearTimers()
          const text = result.text?.trim() ?? ''
          if (text) {
            const textStore = Stores.Chat.$.TextStore
            const current = textStore.getText()
            textStore.setText(current ? `${current} ${text}` : text)
          }
          set({ status: 'idle', elapsedMs: 0, stageText: '', errorMessage: null })
        } catch (err) {
          fail(
            err instanceof Error && err.message
              ? `Transcription failed: ${err.message}`
              : 'Transcription failed.',
          )
        }
    }

    const cancelRecording = () => {
      clearTimers()
      if (mediaRecorder && get().status === 'recording') {
        mediaRecorder.ondataavailable = null
        mediaRecorder.onstop = null
        try {
          mediaRecorder.stop()
        } catch {
          /* already stopped */
        }
      }
      stopStream()
      chunks = []
      set({ status: 'idle', elapsedMs: 0, stageText: '', errorMessage: null })
    }

    return { fetchCapability, startRecording, stopRecording, cancelRecording }
  },
  init: ({ actions }) => {
    void actions.fetchCapability()
  },
})

/** Augment ChatExtensionStores with VoiceStore. */
declare module '../../types' {
  interface ChatExtensionStores {
    VoiceStore: ReturnType<typeof createVoiceStore>
  }
}

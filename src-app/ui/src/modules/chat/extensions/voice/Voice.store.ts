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
let requestTimeout: ReturnType<typeof setTimeout> | null = null
let recordStartedAt = 0
/**
 * Monotonic token guarding the async getUserMedia await. Bumped whenever a
 * pending permission request must be abandoned (cancel / timeout / unmount) so
 * a late-resolving prompt discards its stream instead of latching a mic the
 * user already backed out of. See startRecording / cancelRecording.
 */
let requestGeneration = 0

/** getUserMedia can hang forever on an unanswered permission prompt; escape it. */
const GET_USER_MEDIA_TIMEOUT_MS = 15000

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
  if (requestTimeout !== null) {
    clearTimeout(requestTimeout)
    requestTimeout = null
  }
}

/**
 * Return keyboard focus to the composer textarea after a dictation flow ends
 * (transcript inserted or recording cancelled) — the mic controls unmount on
 * the status change, so without this focus falls to <body>. No-op when the
 * composer isn't mounted (e.g. this ran from an unmount cleanup).
 */
function focusComposer(): void {
  if (typeof document === 'undefined' || typeof requestAnimationFrame === 'undefined') return
  requestAnimationFrame(() => {
    const el = document.querySelector<HTMLTextAreaElement>(
      '[data-testid="chat-message-textarea"]',
    )
    el?.focus()
  })
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
    /** Last error message (surfaced to the user via the persistent live region). */
    errorMessage: null as string | null,
    /**
     * Discrete screen-reader announcement for the single persistent live region
     * in MicButton. Set only on STATE TRANSITIONS ("Recording started",
     * "Transcribing", "Transcript added", "Recording cancelled", or an error
     * message) — never the per-second timer, which must not be re-announced.
     */
    announcement: '',
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
      // Abandon any pending permission prompt whose stream might still resolve.
      requestGeneration++
      stopStream()
      chunks = []
      message.error(msg)
      set({
        status: 'error',
        errorMessage: msg,
        announcement: msg,
        elapsedMs: 0,
        stageText: '',
      })
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
        const gen = ++requestGeneration
        set({
          status: 'requesting',
          errorMessage: null,
          announcement: '',
          elapsedMs: 0,
          stageText: '',
        })
        // Backstop: an unanswered permission prompt never rejects getUserMedia,
        // so time it out and return to idle rather than spinning forever.
        requestTimeout = setTimeout(() => {
          requestTimeout = null
          if (requestGeneration === gen && get().status === 'requesting') {
            fail('Microphone permission timed out. Try again when you’re ready.')
          }
        }, GET_USER_MEDIA_TIMEOUT_MS)
        let stream: MediaStream
        try {
          stream = await navigator.mediaDevices.getUserMedia({ audio: true })
        } catch (err) {
          if (requestTimeout !== null) {
            clearTimeout(requestTimeout)
            requestTimeout = null
          }
          // A cancel/timeout/unmount already superseded this request and reset
          // state — swallow the (now irrelevant) rejection.
          if (requestGeneration !== gen) return
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
        if (requestTimeout !== null) {
          clearTimeout(requestTimeout)
          requestTimeout = null
        }
        // Cancelled/superseded while the prompt was open: discard the live stream
        // instead of leaving the mic on.
        if (requestGeneration !== gen || get().status !== 'requesting') {
          for (const track of stream.getTracks()) track.stop()
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
        set({ status: 'recording', elapsedMs: 0, announcement: 'Recording started' })
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
        // Supersession token, captured BEFORE the finalization await. cancelRecording
        // (Cancel button or the unmount cleanup) bumps requestGeneration; the
        // Recording UI stays live during MediaRecorder's onstop finalization, so a
        // cancel here (or later during the transcribe POST) is a real race. Every
        // await below re-checks this token and bails if superseded.
        const gen = requestGeneration
        // Await the assembled blob via onstop before we tear the stream down. A
        // fallback timeout guarantees the promise settles even if a concurrent
        // cancel nulled `onstop` (otherwise the frame would hang, leaking the
        // closure); double-resolve is a no-op.
        const recorded = await new Promise<Blob>(resolve => {
          const settle = () =>
            resolve(new Blob(chunks, { type: recorder.mimeType || 'audio/webm' }))
          recorder.onstop = settle
          const fallback = setTimeout(settle, 1500)
          void fallback
          try {
            recorder.stop()
          } catch {
            resolve(new Blob(chunks, { type: 'audio/webm' }))
          }
        })
        stopStream()
        chunks = []

        // A cancel/unmount during onstop finalization superseded us — the state
        // was already reset to idle; don't resurrect it into 'transcribing'.
        if (requestGeneration !== gen) return

        if (recorded.size === 0) {
          fail('No audio was captured — try recording again.')
          return
        }

        set({
          status: 'transcribing',
          stageText: 'Starting voice engine…',
          announcement: 'Transcribing',
        })
        // The same `gen` token guards the transcribe POST below: a POST that
        // resolves AFTER a cancel/unmount is dropped instead of appending its
        // transcript into a composer that has since changed. (The fetch itself
        // isn't aborted — the generated client takes no signal — so this is the
        // safety net.)
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
          // A cancel/unmount superseded this transcription while it was in flight
          // — drop the result (state was already reset by cancelRecording).
          if (requestGeneration !== gen) return
          clearTimers()
          const text = result.text?.trim() ?? ''
          if (text) {
            const textStore = Stores.Chat.$.TextStore
            const current = textStore.getText()
            textStore.setText(current ? `${current} ${text}` : text)
          }
          set({
            status: 'idle',
            elapsedMs: 0,
            stageText: '',
            errorMessage: null,
            announcement: text ? 'Transcript added' : 'No speech detected',
          })
          focusComposer()
        } catch (err) {
          // Superseded by a cancel/unmount — swallow the (now irrelevant) error.
          if (requestGeneration !== gen) return
          // Keep the raw backend detail (loopback URLs, engine jargon) in the
          // console for debugging; never leak it into the user-facing toast.
          console.error('[voice] transcription failed', err)
          fail('Couldn’t transcribe your recording. Please try again.')
        }
    }

    const cancelRecording = () => {
      // Nothing to unwind from a settled state — avoid a spurious focus-steal /
      // announcement when this runs as an unmount cleanup while already idle.
      const status = get().status
      if (status === 'idle') return
      clearTimers()
      // Abandon a pending getUserMedia prompt so a late resolve discards its
      // stream instead of re-latching the mic (escapes the 'requesting' spinner).
      requestGeneration++
      if (mediaRecorder && status === 'recording') {
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
      set({
        status: 'idle',
        elapsedMs: 0,
        stageText: '',
        errorMessage: null,
        announcement: 'Recording cancelled',
      })
      focusComposer()
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

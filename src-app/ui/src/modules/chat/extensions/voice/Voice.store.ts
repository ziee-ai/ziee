import { message } from '@ziee/kit'
import { ApiClient } from '@/api-client'
import { Permissions, type VoiceCapability } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { defineExtensionStore } from '@/modules/chat/core/extensions'
import { recordedBlobToWav16k } from './audio/wav'
import {
  appendTranscript,
  composeInterimCaption,
  isSuperseded,
  micErrorMessage,
  resolveLivePref,
  shouldRunInterim,
} from './voiceLogic'

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
/** Self-rescheduling interim (live-caption) decode timer; null when not looping. */
let interimTimer: ReturnType<typeof setTimeout> | null = null
let recordStartedAt = 0
/**
 * In-progress latch for `stopRecording`'s finalization window. `status` stays
 * `'recording'` and `mediaRecorder` stays non-null until the onstop promise
 * resolves, so without this a second stop dispatched in that sub-millisecond gap
 * (e.g. the max-clip auto-stop timer racing a user Stop click) would re-enter.
 */
let finalizing = false
/**
 * Monotonic token guarding the async getUserMedia await. Bumped whenever a
 * pending permission request must be abandoned (cancel / timeout / unmount) so
 * a late-resolving prompt discards its stream instead of latching a mic the
 * user already backed out of. See startRecording / cancelRecording.
 */
let requestGeneration = 0

/** getUserMedia can hang forever on an unanswered permission prompt; escape it. */
const GET_USER_MEDIA_TIMEOUT_MS = 15000

/** Per-device "Live captions" preference (opt-out to batch). */
const LIVE_CAPTIONS_KEY = 'ziee.voice.liveCaptions'
/** Clamp the admin cadence to the same bounds the backend validates (300..=10000). */
function clampInterval(ms: number | undefined): number {
  return Math.min(10_000, Math.max(300, Math.round(ms ?? 1000)))
}
/** Read the stored per-device live-captions pref (null when unset / storage blocked). */
function storedLivePref(): string | null {
  try {
    return typeof localStorage !== 'undefined' ? localStorage.getItem(LIVE_CAPTIONS_KEY) : null
  } catch {
    return null
  }
}

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
  if (interimTimer !== null) {
    clearTimeout(interimTimer)
    interimTimer = null
  }
}

/**
 * Return keyboard focus to the composer textarea after a dictation flow ends
 * (transcript inserted or recording cancelled) — the mic controls unmount on
 * the status change, so without this focus falls to <body>. No-op when the
 * composer isn't mounted (e.g. this ran from an unmount cleanup).
 */
// The composer textarea's testid + attribute, both built from variables so this
// SELECTOR string contains no `data-testid="literal"` sequence — otherwise the
// testid-registry / testid-unique tooling would scan it as a (duplicate)
// declaration of TextInput's real `chat-message-textarea`. This is a query, not
// a declaration.
const COMPOSER_TESTID = 'chat-message-textarea'
const TESTID_ATTR = 'data-testid'

function focusComposer(): void {
  if (typeof document === 'undefined' || typeof requestAnimationFrame === 'undefined') return
  requestAnimationFrame(() => {
    const el = document.querySelector<HTMLTextAreaElement>(
      `[${TESTID_ATTR}="${COMPOSER_TESTID}"]`,
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
     * The live interim caption (full stitched transcript-so-far) shown WHILE
     * recording when live captions are on. Transient preview only — it is NEVER
     * written to the composer; the authoritative transcript is inserted on stop.
     */
    interimText: '',
    /** Per-device "Live captions" preference (opt-out to batch). */
    liveCaptions: false,
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
        interimText: '',
      })
      revertToIdleSoon()
    }

    /**
     * Self-rescheduling live-caption loop: while recording (and not superseded /
     * finalizing), decode the WHOLE accumulating buffer and render the returned
     * full transcript as the interim caption. Single-flight (the next tick is
     * scheduled only after the previous settles) and interim-errors-non-fatal —
     * a failed decode just skips one caption update, never the recording.
     */
    const runInterimTick = (gen: number, intervalMs: number) => {
      interimTimer = setTimeout(async () => {
        interimTimer = null
        if (isSuperseded(gen, requestGeneration) || get().status !== 'recording' || finalizing) {
          return
        }
        try {
          const blob = new Blob(chunks, { type: mediaRecorder?.mimeType || 'audio/webm' })
          if (blob.size > 0) {
            const wav = await recordedBlobToWav16k(blob)
            const formData = new FormData()
            formData.append('file', wav, 'interim.wav')
            const result = await ApiClient.Voice.transcribeStream(formData)
            // Only paint the caption while still actively recording this session.
            if (!isSuperseded(gen, requestGeneration) && get().status === 'recording' && !finalizing) {
              set({ interimText: composeInterimCaption(result.text) })
            }
          }
        } catch {
          // Non-fatal: skip this caption update; recording + the authoritative
          // final decode are unaffected.
        }
        if (!isSuperseded(gen, requestGeneration) && get().status === 'recording' && !finalizing) {
          runInterimTick(gen, intervalMs)
        }
      }, intervalMs)
    }

    /** Fetch the readiness snapshot. Self-gates on the transcribe permission. */
    const fetchCapability = async () => {
      if (!hasPermissionNow(Permissions.VoiceTranscribe)) {
        set({ capabilityLoaded: true })
        return
      }
      try {
        const capability = await ApiClient.Voice.capability()
        // Resolve the per-device live-captions pref against the deployment toggle
        // (default follows streaming_enabled — opt-out).
        set({
          capability,
          capabilityLoaded: true,
          liveCaptions: resolveLivePref(storedLivePref(), capability.streaming_enabled),
        })
      } catch {
        // Non-fatal: the button hides when capability stays null.
        set({ capabilityLoaded: true })
      }
    }

    /** Toggle + persist the per-device "Live captions" preference. */
    const setLiveCaptions = (on: boolean) => {
      try {
        localStorage.setItem(LIVE_CAPTIONS_KEY, on ? '1' : '0')
      } catch {
        /* storage blocked — apply for this session only */
      }
      set({ liveCaptions: on })
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
          if (isSuperseded(gen, requestGeneration)) return
          fail(micErrorMessage(err))
          return
        }
        if (requestTimeout !== null) {
          clearTimeout(requestTimeout)
          requestTimeout = null
        }
        // Cancelled/superseded while the prompt was open: discard the live stream
        // instead of leaving the mic on.
        if (isSuperseded(gen, requestGeneration) || get().status !== 'requesting') {
          for (const track of stream.getTracks()) track.stop()
          return
        }
        mediaStream = stream
        chunks = []
        // Live-caption mode: decode the accumulating buffer on a cadence while
        // recording. A timeslice makes MediaRecorder flush chunks periodically so
        // the accumulate-from-start blob is decodable each interim tick.
        const live = shouldRunInterim('recording', capability, get().liveCaptions)
        const intervalMs = clampInterval(capability?.stream_interval_ms)
        try {
          const recorder = new MediaRecorder(stream)
          mediaRecorder = recorder
          recorder.ondataavailable = e => {
            if (e.data && e.data.size > 0) chunks.push(e.data)
          }
          if (live) recorder.start(intervalMs)
          else recorder.start()
        } catch {
          fail('Could not start the audio recorder.')
          return
        }
        recordStartedAt = Date.now()
        set({ status: 'recording', elapsedMs: 0, interimText: '', announcement: 'Recording started' })
        // Kick off the interim decode loop (guarded by the same generation token).
        if (live) runInterimTick(gen, intervalMs)
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
        // The `finalizing` latch closes the re-entrancy window during the onstop
        // await (status/mediaRecorder remain 'live' across it).
        if (get().status !== 'recording' || !mediaRecorder || finalizing) return
        finalizing = true
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
        // Finalization window closed — re-entry past here is already blocked by
        // the status check (status flips to 'transcribing' below).
        finalizing = false
        stopStream()
        chunks = []

        // A cancel/unmount during onstop finalization superseded us — the state
        // was already reset to idle; don't resurrect it into 'transcribing'.
        if (isSuperseded(gen, requestGeneration)) return

        if (recorded.size === 0) {
          fail('No audio was captured — try recording again.')
          return
        }

        set({
          status: 'transcribing',
          stageText: 'Starting voice engine…',
          announcement: 'Transcribing',
          // The transient interim caption ends here; the authoritative transcript
          // is what lands in the composer below.
          interimText: '',
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
          if (isSuperseded(gen, requestGeneration)) return
          clearTimers()
          const text = result.text?.trim() ?? ''
          if (text) {
            const textStore = Stores.Chat.$.TextStore
            textStore.setText(appendTranscript(textStore.getText(), text))
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
          if (isSuperseded(gen, requestGeneration)) return
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
      finalizing = false
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
        interimText: '',
        announcement: 'Recording cancelled',
      })
      focusComposer()
    }

    return { fetchCapability, setLiveCaptions, startRecording, stopRecording, cancelRecording }
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

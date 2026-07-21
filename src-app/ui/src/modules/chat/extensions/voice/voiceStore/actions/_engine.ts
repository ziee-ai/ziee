import { message } from '@ziee/kit'
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { paneRegistry } from '@/modules/chat/core/stores/chatBridge'
import {
  acquireRecordingLock,
  releaseRecordingLock,
} from '../../voiceRecordingLock'
import { recordedBlobToWav16k } from '../../audio/wav'
import {
  appendTranscript,
  composeInterimCaption,
  isSuperseded,
  micErrorMessage,
  resolveLivePref,
  shouldRunInterim,
} from '../../voiceLogic'
import type { VoiceStoreGet, VoiceStoreSet } from '../state'
import { SplitView } from '@/modules/chat/core/stores/splitView'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/**
 * The voice-dictation state machine — the entire former `Voice.store.ts` action
 * body, relocated VERBATIM. The 5 public actions share per-instance closures
 * (errorRevertTimer, fail, revertToIdleSoon, runInterimTick) + module-scope
 * imperative resources, so they cannot be split into independent per-file
 * factories; instead the engine is built ONCE per store instance (memoized by
 * the stable `get` identity below) and each `actions/<name>.ts` thin-delegates
 * to it. This keeps the delicate supersession-token / recording-lock / per-pane
 * error-revert invariants byte-identical.
 */

// ── Module-scope imperative resources (not reactive / not serializable) ──────
let mediaRecorder: MediaRecorder | null = null
let mediaStream: MediaStream | null = null
let chunks: Blob[] = []
let elapsedTimer: ReturnType<typeof setInterval> | null = null
let stageTimer: ReturnType<typeof setTimeout> | null = null
// NOTE: `errorRevertTimer` is deliberately NOT module-scope — see the per-instance
// declaration inside the engine. The other timers/recorder above ARE shared because
// the exclusive recording lock guarantees at most one pane is in the recording flow
// at a time; but TWO panes can sit in the post-fail 'error' window simultaneously
// (pane A fails → releases the lock → pane B records → fails), so a shared revert
// timer would let pane B's fail cancel pane A's error→idle auto-revert.
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
    return typeof localStorage !== 'undefined'
      ? localStorage.getItem(LIVE_CAPTIONS_KEY)
      : null
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

// The composer textarea's testid + attribute, both built from variables so this
// SELECTOR string contains no `data-testid="literal"` sequence — otherwise the
// testid-registry / testid-unique tooling would scan it as a (duplicate)
// declaration of TextInput's real `chat-message-textarea`. This is a query, not
// a declaration.
const COMPOSER_TESTID = 'chat-message-textarea'
const TESTID_ATTR = 'data-testid'

/** Focus the composer textarea of the OWNING pane, scoped to its `chat-pane-<idx>`
 *  subtree (ITEM-45) — the previous document-wide first-match stole focus to the
 *  leftmost pane. paneId null → single-pane document-wide (unchanged). */
function focusComposer(paneId: string | null): void {
  if (
    typeof document === 'undefined' ||
    typeof requestAnimationFrame === 'undefined'
  )
    return
  requestAnimationFrame(() => {
    let scope: ParentNode = document
    if (paneId) {
      const idx = SplitView.$.panes.findIndex(p => p.paneId === paneId)
      if (idx >= 0) {
        const paneEl = document.querySelector<HTMLElement>(
          `[${TESTID_ATTR}="chat-pane-${idx}"]`,
        )
        if (paneEl) scope = paneEl
      }
    }
    scope
      .querySelector<HTMLTextAreaElement>(
        `[${TESTID_ATTR}="${COMPOSER_TESTID}"]`,
      )
      ?.focus()
  })
}

/** The TextStore of the OWNING pane (ITEM-45) — the pane that recorded — so the
 *  transcript lands in ITS composer, not the focused pane's. paneId null → the
 *  focused/single-pane bridge (unchanged). */
function ownerTextStore(
  paneId: string | null,
): { getText(): string; setText(t: string): void } {
  if (paneId) {
    const handle = paneRegistry.get(paneId)
    if (handle) {
      return (
        handle.api.getState() as unknown as {
          TextStore: { getText(): string; setText(t: string): void }
        }
      ).TextStore
    }
  }
  return Chat.$.TextStore
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

export interface VoiceEngine {
  fetchCapability: () => Promise<void>
  setLiveCaptions: (on: boolean) => void
  startRecording: (paneId?: string | null) => Promise<void>
  stopRecording: () => Promise<void>
  cancelRecording: () => void
}

/** One engine per live store instance, keyed by the stable per-instance `get`. */
const engines = new WeakMap<VoiceStoreGet, VoiceEngine>()

export default function voiceEngine(
  set: VoiceStoreSet,
  get: VoiceStoreGet,
): VoiceEngine {
  const existing = engines.get(get)
  if (existing) return existing

  // Per-instance (per-pane): each pane's 'error' state owns its OWN auto-revert
  // timer, so a second pane failing doesn't cancel this pane's error→idle revert.
  let errorRevertTimer: ReturnType<typeof setTimeout> | null = null
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
    releaseRecordingLock(get().recordingPaneId) // ITEM-45: free the exclusive lock
    message.error(msg)
    set({
      status: 'error',
      recordingPaneId: null,
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
      if (
        isSuperseded(gen, requestGeneration) ||
        get().status !== 'recording' ||
        finalizing
      ) {
        return
      }
      try {
        const blob = new Blob(chunks, {
          type: mediaRecorder?.mimeType || 'audio/webm',
        })
        if (blob.size > 0) {
          const wav = await recordedBlobToWav16k(blob)
          const formData = new FormData()
          formData.append('file', wav, 'interim.wav')
          const result = await ApiClient.Voice.transcribeStream(formData)
          // Only paint the caption while still actively recording this session.
          if (
            !isSuperseded(gen, requestGeneration) &&
            get().status === 'recording' &&
            !finalizing
          ) {
            set({ interimText: composeInterimCaption(result.text) })
          }
        }
      } catch {
        // Non-fatal: skip this caption update; recording + the authoritative
        // final decode are unaffected.
      }
      if (
        !isSuperseded(gen, requestGeneration) &&
        get().status === 'recording' &&
        !finalizing
      ) {
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
        liveCaptions: resolveLivePref(
          storedLivePref(),
          capability.streaming_enabled,
        ),
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

  const startRecording = async (paneId: string | null = null) => {
    const { status, capability } = get()
    if (status !== 'idle' && status !== 'error') return
    if (!isRecordingSupported()) {
      fail('Voice recording is not supported in this browser.')
      return
    }
    // Exclusive recording (ITEM-45, DEC-61 A1): the mic + module recorder are
    // single-owner, so refuse if ANOTHER split pane is already recording rather
    // than clobbering its stream. (Held through transcription so its buffered
    // chunks aren't reset out from under it.)
    if (!acquireRecordingLock(paneId)) {
      set({ announcement: 'Another pane is recording. Stop it first.' })
      return
    }
    const gen = ++requestGeneration
    set({
      status: 'requesting',
      recordingPaneId: paneId,
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
    set({
      status: 'recording',
      elapsedMs: 0,
      interimText: '',
      announcement: 'Recording started',
    })
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
      // Insert into the OWNING pane's composer (ITEM-45), not the focused pane.
      const ownerPaneId = get().recordingPaneId
      if (text) {
        const textStore = ownerTextStore(ownerPaneId)
        textStore.setText(appendTranscript(textStore.getText(), text))
      }
      releaseRecordingLock(ownerPaneId)
      set({
        status: 'idle',
        recordingPaneId: null,
        elapsedMs: 0,
        stageText: '',
        errorMessage: null,
        announcement: text ? 'Transcript added' : 'No speech detected',
      })
      focusComposer(ownerPaneId)
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
    // 'error' is the ONE non-idle state where this pane has ALREADY released the
    // exclusive lock and cleared its recorder ownership (see `fail`). During the
    // ~2.5s error window ANOTHER pane may have acquired the lock and now owns the
    // shared module recorder/stream/chunks/timers. So an 'error'-state cancel
    // (e.g. this pane unmounting) must touch ONLY this pane's own revert timer +
    // status — never the shared recorder, or it would stop the other pane's live
    // recording, wipe its buffer, and (since recordingPaneId is null here)
    // release nothing, stranding that pane's lock forever.
    if (status === 'error') {
      if (errorRevertTimer !== null) {
        clearTimeout(errorRevertTimer)
        errorRevertTimer = null
      }
      set({
        status: 'idle',
        errorMessage: null,
        elapsedMs: 0,
        stageText: '',
        interimText: '',
        announcement: 'Recording cancelled',
      })
      return
    }
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
    const ownerPaneId = get().recordingPaneId
    releaseRecordingLock(ownerPaneId) // ITEM-45: free the exclusive lock
    set({
      status: 'idle',
      recordingPaneId: null,
      elapsedMs: 0,
      stageText: '',
      errorMessage: null,
      interimText: '',
      announcement: 'Recording cancelled',
    })
    focusComposer(ownerPaneId)
  }

  const engine: VoiceEngine = {
    fetchCapability,
    setLiveCaptions,
    startRecording,
    stopRecording,
    cancelRecording,
  }
  engines.set(get, engine)
  return engine
}

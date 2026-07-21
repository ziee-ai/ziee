import type { StoreSet } from '@ziee/framework/store-kit'
import { type VoiceCapability } from '@/api-client/types'

export type VoiceStatus =
  | 'idle'
  | 'requesting'
  | 'recording'
  | 'transcribing'
  | 'error'

/**
 * VoiceStore state — the chat-composer voice-dictation state machine. `immer:
 * false` (non-serializable resources live in module scope, never in state).
 */
export const voiceStoreState = {
  status: 'idle' as VoiceStatus,
  /** The pane that owns the active recording (ITEM-45) — the transcript + focus
   *  target. null on the single-pane route. Kept in per-pane state so this
   *  instance's stop/cancel resolve ITS own owner, not a shared module value. */
  recordingPaneId: null as string | null,
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
}

export type VoiceStoreState = typeof voiceStoreState
export type VoiceStoreSet = StoreSet<VoiceStoreState>
export type VoiceStoreGet = () => VoiceStoreState

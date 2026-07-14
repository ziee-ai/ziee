/**
 * Exclusive voice-recording lock (ITEM-45, DEC-61 A1).
 *
 * The mic is single hardware (one `getUserMedia` stream) + the recorder resources
 * in `Voice.store` are module-level, so recording is physically EXCLUSIVE. This
 * lock tracks which split pane owns the active recording so:
 *   (a) a second pane's `startRecording` is refused rather than clobbering the
 *       shared recorder, and
 *   (b) other panes' mic buttons render disabled/busy while one pane records
 *       (A1 — disable others, the human's choice over "supersede").
 *
 * A tiny `useSyncExternalStore` observable (no full store registration) so the
 * mic button re-renders on acquire/release. `owner` is the recording pane's id;
 * `null` = no active recording. Single-pane recording (no paneId) does not take
 * the lock — there is no other pane to exclude.
 */
import { useSyncExternalStore } from 'react'

let owner: string | null = null
const listeners = new Set<() => void>()
const emit = () => {
  for (const l of listeners) l()
}

/**
 * Try to acquire the lock for `paneId`. Returns true if acquired (or already held
 * by this same pane). A `null` paneId (single-pane) always succeeds WITHOUT taking
 * the lock. Fails only when ANOTHER split pane currently owns it.
 */
export function acquireRecordingLock(paneId: string | null): boolean {
  if (!paneId) return true // single-pane: no cross-pane exclusion needed
  if (owner !== null && owner !== paneId) return false
  owner = paneId
  emit()
  return true
}

/** Release the lock if held by `paneId` (or unconditionally for single-pane null). */
export function releaseRecordingLock(paneId: string | null): void {
  if (!paneId) return
  if (owner === paneId) {
    owner = null
    emit()
  }
}

/** The pane id currently recording, or null. */
export function getRecordingOwner(): string | null {
  return owner
}

export function subscribeRecordingOwner(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

/** Reactively read the recording-owner pane id (re-renders on acquire/release). */
export function useRecordingOwner(): string | null {
  return useSyncExternalStore(
    subscribeRecordingOwner,
    getRecordingOwner,
    getRecordingOwner,
  )
}

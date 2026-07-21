import type { VoiceStoreGet, VoiceStoreSet } from '../state'
import voiceEngine from './_engine'

/** Thin delegate to the per-instance voice engine (see `_engine.ts`). */
export default (set: VoiceStoreSet, get: VoiceStoreGet) =>
  voiceEngine(set, get).fetchCapability

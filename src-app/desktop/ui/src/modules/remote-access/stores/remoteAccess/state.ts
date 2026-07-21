import type { StoreSet } from '@ziee/framework/store-kit'
import type { RemoteAccessStatus, MagicLink } from './types'

export const remoteAccessState = {
  status: null as RemoteAccessStatus | null,
  loading: false,
  saving: false,
  error: null as string | null,
  magicLink: null as MagicLink | null,
  rotationTimer: null as ReturnType<typeof setInterval> | null,
}

export type RemoteAccessState = typeof remoteAccessState
export type RemoteAccessSet = StoreSet<RemoteAccessState>
/** `get()` typed over state + actions — individual action files use this when
 *  they need to call other actions via `get().otherAction()`. The store-kit
 *  runtime spreads lazy dispatchers into the state object, so `get()` at
 *  runtime returns the full merged state + dispatchers. */
export type RemoteAccessGet = () => RemoteAccessState & {
  [key: string]: (...args: any) => any
}

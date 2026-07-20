import { type User } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

/** The Users store state (data fields only — actions live in `./actions/*`,
 *  one async action per file, loaded lazily on first call / `.preload()`). */
export interface UsersState {
  users: User[]
  total: number
  currentPage: number
  pageSize: number
  isInitialized: boolean
  // User registration settings
  userRegistrationEnabled: boolean
  registrationSettingsInitialized: boolean
  loadingRegistrationSettings: boolean
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean
  error: string | null
}

export const usersState: UsersState = {
  users: [],
  total: 0,
  currentPage: 1,
  pageSize: 10,
  isInitialized: false,
  userRegistrationEnabled: true,
  registrationSettingsInitialized: false,
  loadingRegistrationSettings: false,
  loading: false,
  creating: false,
  updating: false,
  deleting: false,
  error: null,
}

/** The `set`/`get` a lazy Users action factory receives. An action file is
 *  `export default (set: UsersSet, get: UsersGet) => async (...) => { … }`. */
export type UsersSet = StoreSet<UsersState>
export type UsersGet = () => UsersState

import type { StoreSet } from '@ziee/framework/store-kit'

export const profileState = { savingProfile: false, savingPassword: false }

export type ProfileState = typeof profileState
export type ProfileSet = StoreSet<ProfileState>
export type ProfileGet = () => ProfileState

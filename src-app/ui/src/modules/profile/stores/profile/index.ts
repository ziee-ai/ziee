import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { profileState, type ProfileState } from './state'
import type { Actions } from './actions.gen'

const ProfileDef = defineStore<ProfileState, Actions>('Profile', {
  immer: true,
  state: profileState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const Profile = registerLazyStore(ProfileDef)
export const useProfileStore = ProfileDef.store

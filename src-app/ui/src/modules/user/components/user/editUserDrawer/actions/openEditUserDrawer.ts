import type { User } from '@/api-client/types'
import type { EditUserDrawerGet, EditUserDrawerSet } from '../state'

export default (set: EditUserDrawerSet, _get: EditUserDrawerGet) =>
  (user: User) => {
    set({ isOpen: true, editingUser: user })
  }

import type { CreateUserDrawerSet } from '../state'

export default (set: CreateUserDrawerSet) => async () => {
  set({ isOpen: false })
}

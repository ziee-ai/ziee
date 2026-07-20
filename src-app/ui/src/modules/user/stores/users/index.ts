import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { usersState, type UsersState } from './state'
import type { Actions } from './actions.gen'

// UNIFIED LAZY STORE PATTERN
// --------------------------
// - Whole-store-lazy: `registerLazyStore` — the store is NOT registered by
//   user/module.tsx; it self-registers the instant its chunk is imported (by a
//   direct-handle consumer or the lazy Users page), so state + every action ride
//   that lazy chunk, off the eager entry chunk.
// - Per-action-lazy: every action is its own file under `./actions/*`, loaded as
//   its own chunk on first call (or `.preload()`). This is what gives per-action
//   desktop override (localOverride shadows one action file) + conflict-free
//   parallel edits (one action = one file).
// - Prefetch gate (`npm run check` → check:action-prefetch): every action must be
//   either invoked in `init` (load-on-mount, already warmed) OR wired to a
//   `.preload()` (hover/intent). The two below with no click trigger are warmed
//   in `init`; the click actions preload from their trigger buttons.
// Actions auto-register from `./actions/*.ts` by filename (no hand-written map).
// Types come from the generated `./actions.gen.ts` (the `Actions` generic).
const UsersStoreDef = defineStore<UsersState, Actions>('Users', {
  state: usersState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, set, actions }) => {
    on('user.updated', event => {
      set(state => ({
        users: state.users.map(u => (u.id === event.data.user.id ? event.data.user : u)),
      }))
    })
    on('user.deleted', event => {
      set(state => ({
        users: state.users.filter(u => u.id !== event.data.userId),
        total: state.total - 1,
      }))
    })
    // Remote sync: loadUsers self-gates on UsersRead.
    const reload = () => void actions.loadUsers()
    on('sync:user', reload)
    on('sync:reconnect', reload)
    void actions.loadUsers()
  },
})

/** Direct-handle proxy — `import { Users }; Users.users` / `Users.loadUsers()`.
 *  Importing this file self-registers the store (so `Stores.Users` resolves too). */
export const Users = registerLazyStore(UsersStoreDef)
/** Raw zustand store (kept for the type augmentation + any raw consumer). */
export const useUsersStore = UsersStoreDef.store

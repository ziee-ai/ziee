import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { usersState } from './state'

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
const UsersStoreDef = defineStore('Users', {
  state: usersState,
  lazyActions: {
    loadUsers: () => import('./actions/loadUsers'),
    createUser: () => import('./actions/createUser'),
    updateUser: () => import('./actions/updateUser'),
    resetUserPassword: () => import('./actions/resetUserPassword'),
    toggleUserActiveStatus: () => import('./actions/toggleUserActiveStatus'),
    deleteUser: () => import('./actions/deleteUser'),
    clearError: () => import('./actions/clearError'),
    loadUserRegistrationSettings: () =>
      import('./actions/loadUserRegistrationSettings'),
    updateUserRegistrationSettings: () =>
      import('./actions/updateUserRegistrationSettings'),
  },
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
    // Programmatic (non-click) actions: warm their chunks so they're ready when
    // fired reactively — and satisfy the prefetch gate without a hover trigger.
    void actions.clearError.preload()
    void actions.loadUserRegistrationSettings.preload()
  },
})

/** Direct-handle proxy — `import { Users }; Users.users` / `Users.loadUsers()`.
 *  Importing this file self-registers the store (so `Stores.Users` resolves too). */
export const Users = registerLazyStore(UsersStoreDef)
/** Raw zustand store (kept for the type augmentation + any raw consumer). */
export const useUsersStore = UsersStoreDef.store

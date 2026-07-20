// UserGroups still eager-registered via user/module.tsx.
export { useUserGroupsStore } from './UserGroups.store'

// NOTE: `useUsersStore` is intentionally NOT re-exported here — a barrel
// re-export would statically re-tether the (now whole-store-lazy) Users store
// into every importer of this barrel, defeating the split. Import the direct
// handle `{ Users }` (or `useUsersStore`) from './Users.store' instead.

// Re-export for compatibility with Stores pattern
export { Stores } from '@ziee/framework/stores'

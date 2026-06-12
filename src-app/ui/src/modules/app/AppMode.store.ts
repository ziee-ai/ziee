import { create } from 'zustand'
import type { StoreProxy } from '@/core/stores'

/**
 * Portable "what kind of build is this" flag for core UI code that
 * needs to branch on multi-user vs single-admin semantics WITHOUT
 * importing from `@ziee/desktop/*` (the desktop-only platform helpers
 * aren't reachable from the core ui package).
 *
 * Default: `multiUserMode: true` (web build).
 * The desktop UI bootstrap (`src-app/desktop/ui/src/main.tsx` or the
 * desktop loader) flips it to `false` at startup.
 *
 * First and currently sole consumer: `SystemMcpServersPage.tsx` —
 * suppresses the per-server `McpServerGroupsAssignmentCard` and the
 * `McpUserPolicyCard` when `multiUserMode === false` (a single-admin
 * desktop has no user groups and no need for a user policy because
 * the single user IS the admin).
 *
 * Reusable for any future in-page widget that should hide on the
 * single-admin desktop.
 */
interface AppModeState {
  multiUserMode: boolean
  setMultiUserMode: (value: boolean) => void
}

declare module '../../core/stores' {
  interface RegisteredStores {
    AppMode: StoreProxy<AppModeState>
  }
}

export const useAppModeStore = create<AppModeState>(set => ({
  multiUserMode: true,
  setMultiUserMode: (value: boolean) => set({ multiUserMode: value }),
}))

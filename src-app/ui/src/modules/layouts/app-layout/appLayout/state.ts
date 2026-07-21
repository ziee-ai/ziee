import type { StoreSet } from '@ziee/framework/store-kit'

export const appLayoutState = {
  // Mobile/responsive state
  isMobile: false,
  isFullscreen: false,
  mobileOverlayOpen: false,
  // Sidebar state
  isSidebarCollapsed: false,
  /**
   * Sidebar width in px. Persisted in the store (NOT a per-mount ref) because
   * each route's `*Layout` mounts its own `<AppLayout>` — a `useRef` would
   * reset on every navigation, wiping the user's resized width.
   */
  sidebarWidth: 240,
  mainContentWidth: 1000,
  /**
   * When true, the app shell unwinds its fixed-height/overflow clamp so the
   * DOCUMENT scrolls instead of an inner scroller (iOS Safari toolbar collapse
   * + content under the notch). Set only by pages that opt in via
   * `useNativeScroll` (Settings, mobile only). Ephemeral (not persisted).
   */
  nativeScroll: false,
  /**
   * In native scroll mode, whether the auto-hiding header is currently hidden.
   * Shared so the fixed sidebar-toggle button disappears/reappears with the
   * header. Ephemeral.
   */
  headerHidden: false,
}

export type AppLayoutState = typeof appLayoutState
export type AppLayoutSet = StoreSet<AppLayoutState>
export type AppLayoutGet = () => AppLayoutState

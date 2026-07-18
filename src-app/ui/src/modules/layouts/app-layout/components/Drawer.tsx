// Shim → @ziee/shell. The core (web) app-wide Drawer primitive moved to
// `@ziee/shell/components/Drawer` (reusable by any SDK-consuming app). This
// `@/`-path shim is kept so:
//   - existing importers stay unchanged, and
//   - the desktop `localOverridePlugin` (which swaps `.desktop` ONLY for `@/`
//     specifiers) still intercepts this path → `Drawer.desktop.tsx` for the
//     Tauri build, which remains app-side (it reaches into `@ziee/desktop` +
//     `@tauri-apps/api` for window-chrome behavior, so it can't live in the SDK).
export { Drawer, type DrawerProps } from '@ziee/shell/components/Drawer'

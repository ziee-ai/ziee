// Shim → @ziee/shell. The core (web) HeaderBarContainer moved to
// `@ziee/shell/components/HeaderBarContainer` (reusable by any SDK-consuming
// app). This `@/`-path shim keeps consumers unchanged and lets the desktop
// `localOverridePlugin` still swap the app-side `HeaderBarContainer.desktop.tsx`
// (Tauri window-drag + traffic-light chrome) for the Tauri build.
export { HeaderBarContainer } from '@ziee/shell/components/HeaderBarContainer'

// Shim → @ziee/shell. The web core moved to `@ziee/shell/hooks/useHeaderLeftInset`
// (reusable by any SDK-consuming app). This `@/`-path shim keeps consumers
// unchanged and lets the desktop `localOverridePlugin` still swap the app-side
// `useHeaderLeftInset.desktop.ts` (macOS traffic-light clearance) for the Tauri build.
export { useHeaderLeftInset } from '@ziee/shell/hooks/useHeaderLeftInset'

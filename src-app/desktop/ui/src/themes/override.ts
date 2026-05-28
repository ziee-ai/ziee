// DELIBERATE DIVERGENCE: desktop-only theme overrides.
//
// Resolved by Vite's localOverridePlugin BEFORE the core
// `src-app/ui/src/themes/override.ts`. Both `light.ts` and `dark.ts`
// from core import their tokens / algorithm via the
// `@/themes/override.ts` alias — when running in the desktop bundle
// the plugin intercepts that alias and serves this file instead, so
// our macOS-tuned values flow through both themes without forking
// the rest of the theme code.
//
// `ComponentOverrides` is re-exported from core unchanged. Typography
// tokens shift to macOS values, and the algorithm drops `compact` so
// the app reads as roomy / native instead of dense / web SaaS.

import { theme } from 'antd'

export { ComponentOverrides } from '@ziee/ui-core/themes/override.ts'

// Compact density on desktop — same as web. Trying it again to see
// whether the chrome reads better tight, given the SF Pro family
// and 14px base. Switch back to `[theme.defaultAlgorithm]` /
// `[theme.darkAlgorithm]` if it feels too cramped vs native Mac
// apps (Notes / Mail / System Settings all run default density).
export const LightAlgorithm = [theme.defaultAlgorithm, theme.compactAlgorithm]
export const DarkAlgorithm = [theme.darkAlgorithm, theme.compactAlgorithm]

// macOS-native stack: `-apple-system` is the AppKit hook that
// resolves to the live system font (SF Pro Text < 20pt, SF Pro
// Display ≥ 20pt) with dynamic optical sizing. Do NOT self-host
// SF Pro — Apple's license forbids it AND the system version gets
// optical sizing the file version doesn't.
const MAC_FONT_STACK = [
  '-apple-system',
  'BlinkMacSystemFont',
  '"SF Pro Text"',
  '"SF Pro"',
  'system-ui',
  'sans-serif',
].join(', ')

export const TokenOverrides = {
  fontFamily: MAC_FONT_STACK,
  // 16px — browser-default. Matches Notes body / iMessage / most
  // macOS reading surfaces. With compactAlgorithm on, spacing
  // stays dense around comfortable Mac-app-sized text.
  fontSize: 16,
  fontSizeIcon: 16,
  borderRadius: 6,
  padding: 6,
}

import { theme } from 'antd'

// Web (cross-OS browser) font stack. `system-ui` resolves to the live
// platform font on every modern browser (SF on macOS, Segoe UI Variable
// on Win11, Roboto on Android/ChromeOS), with the `-apple-system` /
// `BlinkMacSystemFont` aliases kept for older Safari/Chromium.
// No webfont load → zero FOUT, zero network cost.
const WEB_FONT_STACK = [
  'system-ui',
  '-apple-system',
  'BlinkMacSystemFont',
  '"Segoe UI Variable"',
  '"Segoe UI"',
  '"Roboto"',
  '"Helvetica Neue"',
  '"Arial"',
  'sans-serif',
].join(', ')

export const TokenOverrides = {
  fontFamily: WEB_FONT_STACK,
  // 14px is the de-facto standard for dense product UIs (Linear,
  // GitHub, Vercel dashboard). antd's own default is 14; keeping
  // parity avoids the "this looks oversized" perception the prior
  // 16px setting created.
  fontSize: 14,
  fontSizeIcon: 14,
  borderRadius: 6,
  padding: 6,
}

// Theme algorithms. Web ships with compact density (denser product
// UI; matches the Linear/GitHub feel). Desktop's override drops
// compact so the app reads as roomy / native-Mac instead of
// information-dense web SaaS.
export const LightAlgorithm = [theme.defaultAlgorithm, theme.compactAlgorithm]
export const DarkAlgorithm = [theme.darkAlgorithm, theme.compactAlgorithm]

export const ComponentOverrides = {
  Button: {
    // Override button tokens
  },
  Statistic: {
    contentFontSize: 18,
  },
  Card: {
    bodyPadding: 12,
    headerPadding: 12,
  },
  // Other component overrides can go here
}

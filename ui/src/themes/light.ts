import { theme, ThemeConfig } from 'antd'
import tinycolor from 'tinycolor2'

const BaseBackgroundColor = tinycolor('#f0f2f5').lighten(2.5).toString() // Light background color for the light theme

const baseTheme = {
  algorithm: [theme.defaultAlgorithm, theme.compactAlgorithm],
  token: {
    fontSize: 16,
    fontSizeIcon: 16,
    borderRadius: 6,
    padding: 6,
    colorBgLayout: BaseBackgroundColor,
    colorBgContainer: '#ffffff', // Light background for layout
    colorBgBase: BaseBackgroundColor, // Base background color for components
    colorBorder: tinycolor(BaseBackgroundColor).darken(15).toString(),
    colorBorderSecondary: tinycolor(BaseBackgroundColor).darken(7).toString(),
    colorHighlight: tinycolor(BaseBackgroundColor).darken(20).toString(),
    colorBgMask: tinycolor('#f0f2f5').darken(10).setAlpha(0.6).toString(),
    // Fix description text color contrast (WCAG AA requires 4.5:1)
    // Changed from rgba(0,0,0,0.45) [#737373] to #666666 for better contrast
    colorTextDescription: '#666666', // Improves contrast from 3.36 to 5.74
    // Fix link colors for better contrast
    colorLink: '#0958d9', // Primary link color
    colorLinkHover: '#69b1ff', // Keep default hover
    colorLinkActive: '#0958d9', // Keep darker active
  },
  components: {
    Button: {
      // Fix color contrast for primary button (WCAG AA requires 4.5:1)
      // Changed from #1677ff to darker blue for better contrast with white text
      colorPrimary: '#0958d9', // Darker blue improves contrast from 4.1 to 5.2
      colorPrimaryHover: '#1677ff',
      // Fix link color contrast (WCAG AA requires 4.5:1)
      colorLink: '#0958d9', // Darker blue improves contrast from 4.1 to 5.2
      colorLinkHover: '#1677ff',
    },
    Form: {
      // Form inherits colorTextDescription from token
    },
    Typography: {
      // Typography inherits colors from token
    },
    Statistic: {
      contentFontSize: 18,
      // Statistic inherits colorTextDescription from token
    },
    Card: {
      bodyPadding: 12,
      headerPadding: 12,
    },
  },
  app: {
    chatBackground: '#f0f2f5',
  },
} as const

type AppTokenKeys = keyof typeof baseTheme.app
type AppToken = {
  [K in AppTokenKeys]: (typeof baseTheme.app)[K] extends string
    ? string
    : (typeof baseTheme.app)[K] extends number
      ? number
      : (typeof baseTheme.app)[K] extends boolean
        ? boolean
        : (typeof baseTheme.app)[K]
}

export type AppThemeConfig = {
  algorithm: ThemeConfig['algorithm']
  token: ThemeConfig['token']
  components: ThemeConfig['components']
  app: AppToken
}

const lightTheme = baseTheme as unknown as AppThemeConfig

export { lightTheme }

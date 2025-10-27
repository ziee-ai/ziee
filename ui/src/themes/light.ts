import { theme, ThemeConfig } from 'antd'
import { ComponentOverrides, TokenOverrides } from './override.ts'
import tinycolor from 'tinycolor2'

const BaseBackgroundColor = tinycolor('#f0f2f5').lighten(2.5).toString() // Light background color for the light theme

const baseTheme = {
  algorithm: [theme.defaultAlgorithm, theme.compactAlgorithm],
  token: {
    ...TokenOverrides,
    colorBgLayout: BaseBackgroundColor,
    colorBgContainer: '#ffffff', // Light background for layout
    colorBgBase: BaseBackgroundColor, // Base background color for components
    colorBorder: tinycolor(BaseBackgroundColor).darken(15).toString(),
    colorBorderSecondary: tinycolor(BaseBackgroundColor).darken(7).toString(),
    colorHighlight: tinycolor(BaseBackgroundColor).darken(20).toString(),
    colorBgMask: tinycolor('#f0f2f5').darken(10).setAlpha(0.6).toString(),
  },
  components: {
    ...ComponentOverrides,
    Button: {
      ...ComponentOverrides.Button,
      // Override button tokens for light theme
    },
    // Other component overrides can go here
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

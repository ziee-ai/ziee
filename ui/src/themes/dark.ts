import { theme } from 'antd'
import { AppThemeConfig } from './light.ts'
import { ComponentOverrides, TokenOverrides } from './override.ts'
import tinycolor from 'tinycolor2'

const BaseBackgroundColor = '#1e1e1e'

export const darkTheme: AppThemeConfig = {
  algorithm: [theme.darkAlgorithm, theme.compactAlgorithm],
  token: {
    ...TokenOverrides,
    colorBgLayout: BaseBackgroundColor, // Dark background for layout
    colorBgContainer: '#242424',
    colorBgBase: BaseBackgroundColor, // Base background color for components
    colorBorder: tinycolor(BaseBackgroundColor).lighten(15).toString(),
    colorBorderSecondary: tinycolor(BaseBackgroundColor).lighten(7).toString(),
    colorHighlight: tinycolor(BaseBackgroundColor).lighten(20).toString(),
  },
  components: {
    ...ComponentOverrides,
    Button: {
      ...ComponentOverrides.Button,
      // Override button tokens for dark theme
    },
    Modal: {
      contentBg: BaseBackgroundColor,
      footerBg: BaseBackgroundColor,
      headerBg: BaseBackgroundColor,
    },
    // Other component overrides can go here
  },
  app: {
    chatBackground: '#141414', // Dark background for chat
  },
} as const

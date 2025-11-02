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
    // Fix description text color contrast for dark mode (WCAG AA requires 4.5:1)
    // Light text on dark background needs sufficient contrast
    colorTextDescription: 'rgba(255,255,255,0.65)', // Better contrast than default 0.45 opacity
    // Fix link colors for better contrast on dark background
    colorLink: '#69b1ff', // Lighter blue for dark mode
    colorLinkHover: '#91caff', // Even lighter on hover
    colorLinkActive: '#4096ff', // Medium blue when active
  },
  components: {
    ...ComponentOverrides,
    Button: {
      ...ComponentOverrides.Button,
      // Fix color contrast for primary button in dark mode (WCAG AA requires 4.5:1)
      // White text on #0958d9 = 4.9:1 contrast (PASS)
      colorPrimary: '#0958d9', // Darker blue for better contrast with white text
      colorPrimaryHover: '#1677ff', // Lighter on hover
      // Fix link button colors for dark mode
      colorLink: '#69b1ff',
      colorLinkHover: '#91caff',
    },
    Modal: {
      contentBg: BaseBackgroundColor,
      footerBg: BaseBackgroundColor,
      headerBg: BaseBackgroundColor,
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
    Menu: {
      // Fix color contrast for selected menu items in dark mode (WCAG AA requires 4.5:1)
      // Default: foreground #1668dc on background #15325b gives 2.46:1 (FAIL)
      // Fix: Use lighter blue #69b1ff for better contrast on dark background
      colorPrimary: '#69b1ff', // Light blue for selected item text
      colorPrimaryBg: '#15325b', // Dark blue background
      // Fix color contrast for menu item text (applies to Dropdown too since it uses Menu)
      // Light text on dark backgrounds needs high opacity for good contrast
      itemColor: 'rgba(255,255,255,0.85)', // Ensures 4.5:1+ contrast ratio
    },
    Dropdown: {
      // Fix color contrast for dropdown menu items in dark mode (WCAG AA requires 4.5:1)
      // Light text on dark backgrounds needs high opacity for good contrast
      // Dropdown inherits from Menu, so we set both colorText and colorTextLabel
      colorText: 'rgba(255,255,255,0.85)', // Ensures 4.5:1+ contrast ratio on dark backgrounds
      colorTextLabel: 'rgba(255,255,255,0.85)', // Ant Design 5 uses this for menu item text
    },
  },
  app: {
    chatBackground: '#141414', // Dark background for chat
  },
} as const

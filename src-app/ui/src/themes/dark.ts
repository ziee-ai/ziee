import { AppThemeConfig } from '@/themes/light.ts'
import {
  ComponentOverrides,
  DarkAlgorithm,
  TokenOverrides,
} from '@/themes/override.ts'
import tinycolor from 'tinycolor2'

const BaseBackgroundColor = '#1e1e1e'

export const darkTheme: AppThemeConfig = {
  algorithm: DarkAlgorithm,
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
    // Fix global success/error colors for WCAG compliance in dark mode
    // These affect Tag color="success" and color="error"
    colorSuccess: '#95de64', // Light green for dark mode (good contrast on dark bg)
    colorError: '#ff7875', // Light red for dark mode (good contrast on dark bg)
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
      // Fix danger button text color contrast for dark mode (WCAG AA requires 4.5:1)
      // Lighter red for dark mode
      colorError: '#ff7875', // Light red for dark mode
      colorErrorHover: '#ff4d4f', // Slightly darker on hover
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
    Descriptions: {
      // Fix color contrast for description labels in dark mode (WCAG AA requires 4.5:1)
      // Light text on dark background needs sufficient contrast
      labelColor: 'rgba(255,255,255,0.65)', // Light text with good contrast
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
      itemHoverColor: 'rgba(255,255,255,0.85)', // Hover state text
      itemSelectedColor: 'rgba(255,255,255,0.85)', // Selected state text
      // Since Dropdown uses Menu internally, ensure menu items have proper contrast in dark mode
      colorText: 'rgba(255,255,255,0.85)', // Primary text color for menu items
    },
    Dropdown: {
      // Fix color contrast for dropdown menu items in dark mode (WCAG AA requires 4.5:1)
      // Light text on dark backgrounds needs high opacity for good contrast
      // Dropdown inherits from Menu, so we set both colorText and colorTextLabel
      colorText: 'rgba(255,255,255,0.85)', // Ensures 4.5:1+ contrast ratio on dark backgrounds
      colorTextLabel: 'rgba(255,255,255,0.85)', // Ant Design 5 uses this for menu item text
    },
    Tag: {
      // Fix color contrast for green tags in dark mode (WCAG AA requires 4.5:1)
      // Light green text on dark green background
      colorSuccessText: '#95de64', // Light green for dark mode
      colorSuccessBg: '#274916', // Dark green background
      colorSuccessBorder: '#3c6e2f', // Border color
      // Fix color contrast for red tags in dark mode (WCAG AA requires 4.5:1)
      // Light red text on dark red background
      colorErrorText: '#ff7875', // Light red for dark mode
      colorErrorBg: '#321414', // Dark red background
      colorErrorBorder: '#58181c', // Border color
    },
  },
  app: {
    chatBackground: '#141414', // Dark background for chat
  },
} as const

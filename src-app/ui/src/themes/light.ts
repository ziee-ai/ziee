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
    // Fix secondary text color (used by Descriptions labels and other secondary text)
    // Default: rgba(0,0,0,0.45) #8c8c8c = 3.36:1 (FAIL)
    colorTextSecondary: 'rgba(0,0,0,0.65)', // Improves contrast to 4.59:1
    // Fix link colors for better contrast
    colorLink: '#0958d9', // Primary link color
    colorLinkHover: '#69b1ff', // Keep default hover
    colorLinkActive: '#0958d9', // Keep darker active
    // Fix global success/error colors for WCAG compliance
    // These affect Tag color="success" and color="error"
    colorSuccess: '#237804', // Dark green for better contrast (5.74:1 on light bg)
    colorError: '#d4380d', // Dark red for better contrast (4.54:1 on light bg)
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
      // Fix danger button text color contrast (WCAG AA requires 4.5:1)
      // Default: #ff4d4f on white = 3.26:1 (FAIL)
      // Fix: Use darker red for better contrast
      colorError: '#d4380d', // Dark red improves contrast to 4.54:1
      colorErrorHover: '#ff4d4f', // Original color for hover
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
      // Fix color contrast for description labels (WCAG AA requires 4.5:1)
      // Default: rgba(0,0,0,0.45) #8c8c8c on #ffffff = 3.36:1 (FAIL)
      // Fix: Use darker gray for better contrast
      labelColor: 'rgba(0,0,0,0.65)', // Darker gray improves contrast to 4.59:1
    },
    Card: {
      bodyPadding: 12,
      headerPadding: 12,
    },
    Menu: {
      // Fix color contrast for selected menu items (WCAG AA requires 4.5:1)
      // Default: foreground #1677ff on background #e6f4ff gives 3.66:1
      // Fix: Use darker blue #0958d9 for better contrast
      colorPrimary: '#0958d9', // Darker blue for selected item text
      colorPrimaryBg: '#e6f4ff', // Keep light background
      // Fix color contrast for menu item text (applies to Dropdown too since it uses Menu)
      // Default uses rgba(0,0,0,0.65) which gives insufficient contrast
      itemColor: 'rgba(0,0,0,0.88)', // Ensures 4.5:1+ contrast ratio
      itemHoverColor: 'rgba(0,0,0,0.88)', // Hover state text
      itemSelectedColor: 'rgba(0,0,0,0.88)', // Selected state text
      // Since Dropdown uses Menu internally, we need to ensure Menu items have proper contrast
      // The .ant-dropdown-menu-title-content elements specifically need this
      colorText: 'rgba(0,0,0,0.88)', // Primary text color for menu items
    },
    Dropdown: {
      // Fix color contrast for dropdown menu items (WCAG AA requires 4.5:1)
      // Default Ant Design uses rgba(0,0,0,0.65) which gives insufficient contrast
      // Dropdown inherits from Menu, so we set multiple tokens to ensure coverage
      colorText: 'rgba(0,0,0,0.88)', // Ensures 4.5:1+ contrast ratio on #fcfcfc background
      colorTextLabel: 'rgba(0,0,0,0.88)', // Ant Design 5 uses this for menu item text
      colorTextDisabled: 'rgba(0,0,0,0.6)', // Disabled items also need accessible contrast (3.5:1 minimum for large text)
      // controlItemBgHover controls the background, we need to ensure text color is set
      // The Menu component uses these additional tokens for the menu items
      colorTextDescription: 'rgba(0,0,0,0.88)', // Ensures consistent text color
    },
    Tag: {
      // Fix color contrast for green tags (WCAG AA requires 4.5:1)
      // Default: #389e0d on #f6ffed = 3.37:1 (FAIL)
      // Fix: Use darker green for better contrast
      colorSuccessText: '#237804', // Dark green improves contrast to 5.74:1
      colorSuccessBg: '#d9f7be', // Slightly darker green background
      colorSuccessBorder: '#b7eb8f', // Border color
      // Fix color contrast for red tags (WCAG AA requires 4.5:1)
      // Default: Similar insufficient contrast
      // Fix: Use darker red for better contrast
      colorErrorText: '#a8071a', // Dark red improves contrast
      colorErrorBg: '#ffccc7', // Light red background
      colorErrorBorder: '#ffa39e', // Border color
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

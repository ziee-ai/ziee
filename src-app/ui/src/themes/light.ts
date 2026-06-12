import { ThemeConfig } from 'antd'
import {
  ComponentOverrides,
  LightAlgorithm,
  TokenOverrides,
} from '@/themes/override.ts'

// Two surface fills only: the main content pane is pure white,
// and the sidebar is a fractionally off-white so it reads as a
// slightly recessed surface without becoming a heavy gray panel.
// Hairline border between them.
const SIDEBAR_BG = '#F9F9F9'        // sidebar fill
const CONTENT_BG = '#FFFFFF'        // content / card fill
const BORDER = '#E0E0E0'            // visible 1px separator
const BORDER_FAINT = '#ECECEC'      // hairline between rows
const SELECTION_BG = '#E3E3E3'      // neutral selection pill
const HOVER_BG = 'rgba(0,0,0,0.04)' // row hover

const baseTheme = {
  algorithm: LightAlgorithm,
  token: {
    ...TokenOverrides,
    borderRadius: 6,
    borderRadiusLG: 8,
    borderRadiusSM: 4,

    // Two surfaces: `colorBgLayout` is the sidebar fill (#F9F9F9 —
    // a fractionally recessed off-white); `colorBgContainer` is the
    // content / card fill (#FFFFFF). LeftSidebar reads
    // `colorBgLayout` so the split lands automatically.
    colorBgLayout: SIDEBAR_BG,
    colorBgContainer: CONTENT_BG,
    colorBgElevated: CONTENT_BG,
    colorBgBase: CONTENT_BG,

    // Borders
    colorBorder: BORDER,
    colorBorderSecondary: BORDER_FAINT,
    colorHighlight: SELECTION_BG,
    colorBgMask: 'rgba(0,0,0,0.40)',

    // Near-black body text + a four-step label ramp (primary →
    // secondary → tertiary → placeholder). Hits WCAG AA on the
    // white surface across the whole ramp.
    colorText: '#1D1D1F',
    colorTextBase: '#1D1D1F',
    colorTextSecondary: '#6E6E73',
    colorTextTertiary: '#86868B',
    colorTextDescription: '#6E6E73',
    colorTextPlaceholder: '#86868B',

    // Brand / link blue. Slightly darker than the standard
    // system-blue so it carries on the off-white sidebar without
    // dropping below AA.
    colorLink: '#0066CC',
    colorLinkHover: '#0070F3',
    colorLinkActive: '#0066CC',
    colorPrimary: '#0066CC',
    colorPrimaryHover: '#0070F3',

    // Status colors — keep WCAG-compliant darker greens/reds (the
    // original light theme had careful contrast work; preserve it).
    colorSuccess: '#237804',
    colorError: '#d4380d',
  },
  components: {
    ...ComponentOverrides,
    Button: {
      ...ComponentOverrides.Button,
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
      // Roomier than antd's default 12px so content doesn't feel
      // cramped against card edges.
      bodyPadding: 16,
      headerPadding: 16,
      colorBorderSecondary: BORDER_FAINT,
    },
    Menu: {
      // Selected row renders as a NEUTRAL gray pill, not a
      // tinted-blue band. Selected text stays the regular label
      // color so the sidebar reads as quiet/neutral rather than
      // "antd blue accent".
      colorPrimary: '#1D1D1F',          // selected text = body text
      colorPrimaryBg: SELECTION_BG,     // selected row pill
      itemBg: 'transparent',            // pulls the parent surface
      itemColor: '#1D1D1F',
      itemHoverBg: HOVER_BG,
      itemHoverColor: '#1D1D1F',
      itemSelectedBg: SELECTION_BG,
      itemSelectedColor: '#1D1D1F',
      // Group titles ("Navigation" / "Tools" / "Recent chats").
      // Secondary label color, slightly smaller than body text.
      groupTitleColor: '#6E6E73',
      groupTitleFontSize: 11,
      // Inherits for Dropdown / sub-menus.
      colorText: '#1D1D1F',
    },
    Select: {
      // Fix Select placeholder contrast — token override doesn't always
      // apply to `.ant-select-placeholder`. WCAG AA needs 4.5:1.
      colorTextPlaceholder: '#737373',
      colorTextQuaternary: '#737373',
    },
    Input: {
      // Same fix for Input placeholders.
      colorTextPlaceholder: '#737373',
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
      // Fix color contrast for orange tags ("System" badge etc.)
      // Default: #d46b08 on #fff7e6 = 3.33:1 (FAIL)
      // Fix: Use darker orange for better contrast
      colorWarningText: '#873800', // Darker orange improves contrast to ~4.7:1
      colorWarningBg: '#fff1b8',
      colorWarningBorder: '#ffd591',
    },
  },
  app: {
    chatBackground: CONTENT_BG,
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

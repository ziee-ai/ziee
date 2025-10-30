# Theme Persistence Implementation Plan

## Overview
Implement localStorage-based theme persistence with UI controls in the General Settings module.

## Current State

### Existing Infrastructure
- ✅ `ThemeProvider` component (`src/components/ThemeProvider/ThemeProvider.tsx`)
- ✅ Light theme config (`src/themes/light.ts`) - with accessibility fixes
- ✅ Dark theme config (`src/themes/dark.ts`) - needs accessibility fixes
- ✅ Theme context (`src/hooks/useTheme.ts`)
- ✅ System preference detection (`src/components/ThemeProvider/resolveTheme.ts`)
- ✅ General Settings module (`src/modules/settings-general/`)

### Current Limitations
- ❌ Theme preference hardcoded to `'light'` (ThemeProvider.tsx:16)
- ❌ No localStorage persistence
- ❌ No user-facing theme toggle UI
- ❌ Dark theme missing accessibility improvements
- ❌ useTheme hook only provides theme config, not control functions

## Implementation Plan

### Phase 1: Fix Dark Theme Accessibility

**File**: `src/themes/dark.ts`

**Changes**:
1. Keep import of `TokenOverrides` and `ComponentOverrides` from `override.ts` (for global settings)
2. Add dark-mode-specific accessibility improvements:
   - Set `colorTextDescription` for better contrast (light text on dark background)
   - Set link colors with proper contrast (`colorLink`, `colorLinkHover`, `colorLinkActive`)
   - Set Button colors with proper contrast in the components section
3. Add comments documenting WCAG AA compliance

**Note**: `override.ts` contains global settings shared by both light and dark themes. Only add theme-specific color overrides in `dark.ts`.

**Expected Contrast Ratios** (WCAG AA requires 4.5:1):
- Description text on dark background: > 4.5:1
- Links on dark background: > 4.5:1
- Button text on primary button: > 4.5:1

---

### Phase 2: Create Config-Client Module with Zustand Store

**New Module**: `src/modules/config-client/`

Following the same pattern as the auth module, create a config-client module that manages client-side configuration using Zustand with localStorage persistence.

**File**: `src/modules/config-client/store.ts`

```typescript
import { create } from 'zustand'
import { persist, subscribeWithSelector } from 'zustand/middleware'
import type { StoreProxy } from '@/core/stores'

export type ThemePreference = 'light' | 'dark' | 'system'

interface ConfigClientState {
  themePreference: ThemePreference
}

// Augment RegisteredStores for IntelliSense
declare module '../../core/stores' {
  interface RegisteredStores {
    ConfigClient: StoreProxy<ConfigClientState>
  }
}

const defaultState: ConfigClientState = {
  themePreference: 'system',
}

export const useConfigClientStore = create<ConfigClientState>()(
  subscribeWithSelector(
    persist((): ConfigClientState => defaultState, {
      name: 'config-client-storage',
      partialize: state => ({ themePreference: state.themePreference }),
    }),
  ),
)

// Config actions
export const setThemePreference = (preference: ThemePreference): void => {
  useConfigClientStore.setState({ themePreference: preference })
}

export const getThemePreference = (): ThemePreference => {
  return useConfigClientStore.getState().themePreference
}
```

**File**: `src/modules/config-client/module.tsx`

```typescript
import { createModule } from '@/core'
import { useConfigClientStore } from './store'

export default createModule({
  metadata: {
    name: 'config-client',
    version: '1.0.0',
    description: 'Client-side configuration management',
  },
  stores: [
    {
      name: 'ConfigClient',
      store: useConfigClientStore,
    },
  ],
  initialize: () => {
    console.log('Config-client module initialized')
  },
})
```

**Purpose**:
- Use Zustand for state management (consistent with auth module)
- Automatic localStorage persistence via `persist` middleware
- Only persist `themePreference` field
- Type-safe store with RegisteredStores augmentation
- Easy to extend with more client config later (language, font size, etc.)

---

### Phase 3: Update ThemeProvider to Use Config-Client Store

**File**: `src/components/ThemeProvider/ThemeProvider.tsx`

**Changes**:

1. **Import Stores API and setThemePreference action**:
   ```typescript
   import { Stores } from '@/core/stores'
   import { setThemePreference } from '@/modules/config-client/store'
   ```

2. **Replace hardcoded state with registered store**:
   ```typescript
   const selectedTheme = Stores.ConfigClient.use(state => state.themePreference)
   ```

3. **Update context to provide control functions**:
   ```typescript
   <ThemeContext.Provider value={{
     currentTheme,
     selectedTheme,
     setTheme: setThemePreference,
     isDarkMode,
     resolvedTheme
   }}>
   ```

**Key Features**:
- Use registered store via `Stores.ConfigClient` (proper module pattern)
- Automatically persisted via Zustand persist middleware
- No manual localStorage management needed
- Preserve existing system preference detection
- Reactive updates via store selector

---

### Phase 4: Update useTheme Hook

**File**: `src/hooks/useTheme.ts`

**Changes**:

```typescript
import { createContext, useContext } from 'react'
import { AppThemeConfig } from '@/themes/light'

export type ThemePreference = 'light' | 'dark' | 'system'
export type ThemeName = 'light' | 'dark'

export interface ThemeContextValue {
  currentTheme: AppThemeConfig
  selectedTheme: ThemePreference
  resolvedTheme: ThemeName
  isDarkMode: boolean
  setTheme: (theme: ThemePreference) => void
}

export const ThemeContext = createContext<ThemeContextValue | undefined>(undefined)

export function useTheme() {
  const context = useContext(ThemeContext)
  if (!context) {
    throw new Error('useTheme must be used within ThemeProvider')
  }
  return context
}
```

**Purpose**:
- Provide type-safe access to theme state
- Expose theme control functions
- Provide computed values (isDarkMode, resolvedTheme)

---

### Phase 5: Create Theme Settings UI Component

**New File**: `src/modules/settings-general/components/ThemeSettings.tsx`

```typescript
import { Card, Radio, Space, Typography } from 'antd'
import { Stores } from '@/core/stores'
import { setThemePreference } from '@/modules/config-client/store'
import { MdLightMode, MdDarkMode, MdSettingsBrightness } from 'react-icons/md'

const { Title, Text } = Typography

export function ThemeSettings() {
  const themePreference = Stores.ConfigClient.use(state => state.themePreference)

  return (
    <Card>
      <Space direction="vertical" size="middle" style={{ width: '100%' }}>
        <div>
          <Title level={5}>Theme</Title>
          <Text type="secondary">
            Choose how the app looks. Select a single theme or sync with your system.
          </Text>
        </div>

        <Radio.Group
          value={themePreference}
          onChange={(e) => setThemePreference(e.target.value)}
        >
          <Space direction="vertical" size="small">
            <Radio value="light">
              <Space>
                <MdLightMode />
                <span>Light</span>
              </Space>
            </Radio>
            <Radio value="dark">
              <Space>
                <MdDarkMode />
                <span>Dark</span>
              </Space>
            </Radio>
            <Radio value="system">
              <Space>
                <MdSettingsBrightness />
                <span>System</span>
              </Space>
            </Radio>
          </Space>
        </Radio.Group>
      </Space>
    </Card>
  )
}
```

**Features**:
- Radio group for theme selection
- Icons for visual clarity
- Description text for user guidance
- Immediate theme switching on selection
- Uses registered store via `Stores.ConfigClient` (proper module pattern)

---

### Phase 6: Integrate ThemeSettings into GeneralSettings

**File**: `src/modules/settings-general/GeneralSettings.tsx`

**Changes**:

```typescript
import { ThemeSettings } from './components/ThemeSettings'

export default function GeneralSettings() {
  return (
    <div className="h-full overflow-y-auto p-6">
      <Space direction="vertical" size="large" style={{ width: '100%' }}>
        <ThemeSettings />

        {/* Future settings cards go here */}
      </Space>
    </div>
  )
}
```

**Purpose**:
- Display theme settings as first option
- Prepare for additional settings sections

---

## File Structure

```
src/
├── components/
│   └── ThemeProvider/
│       ├── ThemeProvider.tsx        [MODIFY] Use config-client store
│       └── resolveTheme.ts          [NO CHANGE]
├── hooks/
│   └── useTheme.ts                  [MODIFY] Add control functions + types
├── modules/
│   ├── config-client/               [NEW MODULE]
│   │   ├── store.ts                 [NEW] Zustand store with persist
│   │   └── module.tsx               [NEW] Module registration
│   └── settings-general/
│       ├── components/
│       │   └── ThemeSettings.tsx    [NEW] Theme selection UI
│       ├── GeneralSettings.tsx      [MODIFY] Add ThemeSettings component
│       └── module.tsx               [NO CHANGE]
└── themes/
    ├── dark.ts                      [MODIFY] Add accessibility fixes
    ├── light.ts                     [NO CHANGE] Already has fixes
    ├── index.ts                     [NO CHANGE]
    └── override.ts                  [NO CHANGE] Global overrides
```

---

## Implementation Order

1. **Phase 1**: Fix dark theme accessibility
   - Update `dark.ts` with proper contrast colors for dark mode
   - Add comments documenting WCAG compliance
   - Keep `override.ts` imports (global settings)

2. **Phase 2**: Create config-client module
   - Create `src/modules/config-client/store.ts` with Zustand store
   - Create `src/modules/config-client/module.tsx` for module registration
   - Module will be automatically registered (no manual registration needed)

3. **Phase 3**: Update ThemeProvider
   - Import `useConfigClientStore` and `setThemePreference`
   - Replace hardcoded state with Zustand store selector
   - Pass `setThemePreference` to context

4. **Phase 4**: Update useTheme hook
   - Add proper TypeScript types for context value
   - Export control functions interface

5. **Phase 5**: Create ThemeSettings UI
   - Create `src/modules/settings-general/components/` directory
   - Create `ThemeSettings.tsx` component
   - Add radio group with icons (Light/Dark/System)
   - Use config-client store directly

6. **Phase 6**: Integrate into GeneralSettings
   - Import and render ThemeSettings
   - Update layout with proper spacing

---

## Testing Plan

### Manual Testing
1. **Initial Load**:
   - Clear localStorage
   - Verify default is 'system'
   - Verify theme matches system preference

2. **Theme Selection**:
   - Navigate to Settings > General
   - Select Light → verify theme changes + localStorage updates
   - Select Dark → verify theme changes + localStorage updates
   - Select System → verify theme matches system preference

3. **Persistence**:
   - Select Light theme
   - Refresh page → verify Light theme persists
   - Select Dark theme
   - Close and reopen app → verify Dark theme persists

4. **System Preference**:
   - Select System theme
   - Change OS theme preference → verify app theme updates

5. **Accessibility**:
   - Run axe-core checks on both themes
   - Verify all contrast ratios meet WCAG AA

### E2E Testing (Optional Future Enhancement)
- Add test for theme persistence across page refreshes
- Add test for theme switching in settings
- Add accessibility checks for dark theme

---

## Benefits

1. **User Experience**:
   - Persistent theme preference across sessions
   - Respects system preferences by default
   - Easy-to-find settings in General Settings

2. **Accessibility**:
   - Both themes meet WCAG AA standards
   - Dark mode reduces eye strain in low light
   - System preference support for accessibility needs

3. **Developer Experience**:
   - Type-safe theme utilities
   - Centralized localStorage logic
   - Easy to extend with more themes

4. **Performance**:
   - No backend calls required
   - Instant theme switching
   - Minimal localStorage operations

---

## Future Enhancements

1. **Theme Customization**:
   - Allow custom accent colors
   - Save custom themes to localStorage

2. **Theme Schedule**:
   - Auto-switch based on time of day
   - Custom schedule (e.g., dark mode 6pm-6am)

3. **Per-Module Themes**:
   - Different themes for different sections
   - Chat-specific theme preferences

4. **Theme Preview**:
   - Live preview before applying
   - Sample UI in settings panel

---

## Dependencies

**Existing**:
- Ant Design v5 (theme system)
- tinycolor2 (color manipulation)
- react-icons (icon components)

**No new dependencies required**

---

## Risk Assessment

**Low Risk**:
- All changes are additive (no breaking changes)
- localStorage is widely supported
- Graceful fallback to 'system' on errors

**Edge Cases Handled**:
- localStorage quota exceeded → catch and log error
- Invalid stored values → default to 'system'
- No system preference → default to 'light'
- Context used outside provider → throw helpful error

---

## Rollback Plan

If issues arise:
1. Reset ThemeProvider to hardcoded 'light' theme
2. Remove ThemeSettings component from GeneralSettings
3. Previous behavior restored (always light theme)

No database migrations or backend changes required for rollback.

/**
 * The visual-test matrix — the single source of truth for the theme × accent ×
 * viewport combinations the gallery is rendered under. Imported by both the
 * gallery (control bar + URL handling) and the Playwright Layer-B spec so the
 * two never drift.
 */
import {
  ACCENT_ORDER,
  type AccentPreset,
} from '@/components/ThemeProvider/accentPresets'
import type { ThemePreference } from '@/modules/config-client/ConfigClient.store'

export type GalleryTheme = 'light' | 'dark'
export const GALLERY_THEMES: GalleryTheme[] = ['light', 'dark']

/**
 * Accents the screenshot matrix sweeps. The full preset list is selectable in
 * the control bar, but the snapshot matrix uses a representative subset
 * (default + two structurally-distinct hues) to bound baseline count — the
 * point is to prove components hold under *any* accent, not to snapshot all 8.
 */
export const GALLERY_MATRIX_ACCENTS: AccentPreset[] = ['blue', 'teal', 'rose']

/** Every accent (for the manual control bar). */
export const GALLERY_ALL_ACCENTS: AccentPreset[] = [...ACCENT_ORDER]

export interface GalleryViewport {
  name: 'mobile' | 'tablet' | 'desktop'
  width: number
  height: number
}

export const GALLERY_VIEWPORTS: GalleryViewport[] = [
  { name: 'mobile', width: 390, height: 844 },
  { name: 'tablet', width: 768, height: 1024 },
  { name: 'desktop', width: 1280, height: 900 },
]

export const GALLERY_PATH = '/dev/gallery'
/** Standalone (backend-free) Vite entry served in dev. */
export const GALLERY_STANDALONE_PATH = '/dev-gallery.html'

export interface GalleryParams {
  theme: GalleryTheme
  accent: AccentPreset
}

/** Parse `?theme=&accent=` into a validated {theme, accent} (with defaults). */
export function parseGalleryParams(search: string): GalleryParams {
  const q = new URLSearchParams(search)
  const theme = q.get('theme')
  const accent = q.get('accent')
  return {
    theme: theme === 'dark' ? 'dark' : 'light',
    accent: (GALLERY_ALL_ACCENTS as string[]).includes(accent ?? '')
      ? (accent as AccentPreset)
      : 'blue',
  }
}

/** Theme param → ConfigClient preference (gallery forces an explicit theme). */
export function themeToPreference(theme: GalleryTheme): ThemePreference {
  return theme
}

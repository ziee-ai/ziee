/**
 * Drives the gallery's theme + accent from the URL (`?theme=&accent=`) so the
 * WHOLE gallery re-renders per combo — the mechanism Layer B uses to sweep the
 * matrix, and the manual reviewer uses via `/dev/gallery?theme=dark&accent=teal`.
 *
 * It writes through the REAL `ConfigClient` store, so the app `ThemeProvider`
 * applies tokens/theme/accent exactly as in production (no parallel theming
 * path). Works both in-app (ConfigClient registered by its module) and in the
 * standalone entry (registered by `main.tsx`).
 */
import { useCallback, useEffect, useState } from 'react'
import { Stores } from '@/core/stores'
import type { AccentPreset } from '@/components/ThemeProvider/accentPresets'
import {
  type GalleryParams,
  type GalleryTheme,
  parseGalleryParams,
  themeToPreference,
} from './matrix'

function applyToStore(p: GalleryParams) {
  Stores.ConfigClient.setThemePreference(themeToPreference(p.theme))
  Stores.ConfigClient.setAccentPreset(p.accent)
}

export function useGalleryTheme() {
  const [params, setParams] = useState<GalleryParams>(() =>
    parseGalleryParams(window.location.search),
  )

  // Apply on mount + whenever the URL params change.
  useEffect(() => {
    applyToStore(params)
  }, [params.theme, params.accent])

  // Keep in sync with back/forward navigation.
  useEffect(() => {
    const onPop = () => setParams(parseGalleryParams(window.location.search))
    window.addEventListener('popstate', onPop)
    return () => window.removeEventListener('popstate', onPop)
  }, [])

  const setTheme = useCallback((theme: GalleryTheme) => {
    setParams(prev => {
      const next = { ...prev, theme }
      writeUrl(next)
      return next
    })
  }, [])

  const setAccent = useCallback((accent: AccentPreset) => {
    setParams(prev => {
      const next = { ...prev, accent }
      writeUrl(next)
      return next
    })
  }, [])

  return { params, setTheme, setAccent }
}

function writeUrl(p: GalleryParams) {
  const url = new URL(window.location.href)
  url.searchParams.set('theme', p.theme)
  url.searchParams.set('accent', p.accent)
  window.history.replaceState(null, '', url.toString())
}

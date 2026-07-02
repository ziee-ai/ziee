/**
 * Shared helpers for the gallery visual specs.
 *
 * The matrix constants mirror `src/dev/gallery/matrix.ts` (the source of truth);
 * they're inlined here so the spec stays decoupled from the app module graph
 * (importing the gallery would pull React/CSS/`@/` aliases into the test runner).
 */
import type { Page } from '@playwright/test'

export const STANDALONE_PATH = '/dev-gallery.html'

/**
 * Layer B (pixel screenshots) needs blessed, environment-specific baselines, so
 * it's OPT-IN: set VISUAL_SNAPSHOTS=1 (e.g. when blessing in a pinned container)
 * to run the toHaveScreenshot assertions. CI runs the deterministic Layer-A
 * layout + axe checks (incl. the open-overlay and hover/focus layout assertions)
 * with no baseline, and skips the pixel comparisons.
 */
export const SNAPSHOTS_ENABLED = !!process.env.VISUAL_SNAPSHOTS

export const THEMES = ['light', 'dark'] as const
export type Theme = (typeof THEMES)[number]

/**
 * The snapshot matrix sweeps EVERY user-selectable accent — i.e. the full
 * `ACCENT_ORDER` the Settings → Appearance picker offers (accentPresets.ts).
 * Keep this list in lockstep with that picker. For a fast local run, override
 * with VISUAL_ACCENTS="blue,teal" (comma-separated keys).
 */
export const ALL_ACCENTS = [
  'blue',
  'indigo',
  'slate',
  'teal',
  'green',
  'violet',
  'rose',
  'amber',
  'black',
] as const
export type Accent = (typeof ALL_ACCENTS)[number]

export const MATRIX_ACCENTS: readonly string[] = process.env.VISUAL_ACCENTS
  ? process.env.VISUAL_ACCENTS.split(',').map(s => s.trim())
  : ALL_ACCENTS

export const VIEWPORTS = [
  { name: 'mobile', width: 390, height: 844 },
  { name: 'tablet', width: 768, height: 1024 },
  { name: 'desktop', width: 1280, height: 900 },
] as const

export function galleryUrl(theme: string, accent: string, dir = 'ltr'): string {
  return `${STANDALONE_PATH}?theme=${theme}&accent=${accent}&dir=${dir}`
}

/** Navigate to the gallery under a theme/accent/dir and wait for it to settle. */
export async function openGallery(
  page: Page,
  theme: string,
  accent: string,
  dir: 'ltr' | 'rtl' = 'ltr',
): Promise<void> {
  await page.goto(galleryUrl(theme, accent, dir))
  await page.getByTestId('gallery-root').waitFor({ state: 'visible' })
  // The theme CLASS is pre-painted by the inline script, but the ACCENT
  // (--primary) is applied later in ThemeProvider's effect. Wait on a positive
  // accent signal — --primary present + the dir applied — so a screenshot can't
  // capture a pre-accent frame.
  await page.waitForFunction(
    t => {
      const root = document.documentElement
      const primary = getComputedStyle(root).getPropertyValue('--primary').trim()
      return root.classList.contains(t.theme) && root.dir === t.dir && primary.length > 0
    },
    { theme, dir },
  )
  await page.evaluate(() => document.fonts?.ready)
}

/** Every `gallery-section-*` testid currently in the DOM, in document order. */
export async function sectionTestIds(page: Page): Promise<string[]> {
  return page.$$eval('[data-testid^="gallery-section-"]', els =>
    els.map(el => el.getAttribute('data-testid')!).filter(Boolean),
  )
}

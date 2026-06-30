/**
 * Shared helpers for the gallery visual specs.
 *
 * The matrix constants mirror `src/dev/gallery/matrix.ts` (the source of truth);
 * they're inlined here so the spec stays decoupled from the app module graph
 * (importing the gallery would pull React/CSS/`@/` aliases into the test runner).
 */
import type { Page } from '@playwright/test'

export const STANDALONE_PATH = '/dev-gallery.html'

export const THEMES = ['light', 'dark'] as const
export type Theme = (typeof THEMES)[number]

/** Representative accent subset for the snapshot matrix (default + two hues). */
export const MATRIX_ACCENTS = ['blue', 'teal', 'rose'] as const
export type Accent = (typeof MATRIX_ACCENTS)[number]

export const VIEWPORTS = [
  { name: 'mobile', width: 390, height: 844 },
  { name: 'tablet', width: 768, height: 1024 },
  { name: 'desktop', width: 1280, height: 900 },
] as const

export function galleryUrl(theme: string, accent: string): string {
  return `${STANDALONE_PATH}?theme=${theme}&accent=${accent}`
}

/** Navigate to the gallery under a theme/accent and wait for it to settle. */
export async function openGallery(
  page: Page,
  theme: string,
  accent: string,
): Promise<void> {
  await page.goto(galleryUrl(theme, accent))
  await page.getByTestId('gallery-root').waitFor({ state: 'visible' })
  // Let the ThemeProvider apply the resolved theme/accent + fonts settle.
  await page.waitForFunction(
    t => document.documentElement.classList.contains(t),
    theme,
  )
  await page.evaluate(() => document.fonts?.ready)
}

/** Every `gallery-section-*` testid currently in the DOM, in document order. */
export async function sectionTestIds(page: Page): Promise<string[]> {
  return page.$$eval('[data-testid^="gallery-section-"]', els =>
    els.map(el => el.getAttribute('data-testid')!).filter(Boolean),
  )
}

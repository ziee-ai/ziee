/**
 * TEST-9 (ITEM-3/4/5/6/8/9/10/11) — migration parity: after moving all overlay /
 * deep / seeded entries out of the central files into per-module `gallery.tsx`,
 * the gallery's runtime registry must still enumerate EVERY pre-migration surface
 * (a committed 155-slug baseline) with no duplicates. A dropped/renamed slug =
 * lost coverage.
 */
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { test, expect } from '@playwright/test'

const baseline: string[] = JSON.parse(
  readFileSync(
    fileURLToPath(new URL('./__fixtures__/gallery-seed-baseline.json', import.meta.url)),
    'utf-8',
  ),
)

type Surfaces = {
  pages: string[]
  overlays: string[]
  deep: string[]
  seeded: string[]
  interactions: { slug: string; name: string }[]
}

test('TEST-9: every pre-migration overlay/deep/seeded slug is still registered', async ({
  page,
}) => {
  await page.goto('/gallery.html?theme=light&accent=blue')
  await page.getByTestId('gallery-root').waitFor()

  const surfaces = await page.evaluate(
    () =>
      (
        window as unknown as {
          __GALLERY_LIST_ALL_SURFACES__: () => Surfaces
        }
      ).__GALLERY_LIST_ALL_SURFACES__(),
  )

  const registered = new Set([
    ...surfaces.overlays,
    ...surfaces.deep,
    ...surfaces.seeded,
  ])

  const missing = baseline.filter(s => !registered.has(s))
  expect(missing, `slugs lost in migration: ${missing.join(', ')}`).toEqual([])

  // No duplicate slug across the three interaction-only classes.
  const all = [...surfaces.overlays, ...surfaces.deep, ...surfaces.seeded]
  expect(all.length, 'duplicate slug across overlay/deep/seeded').toBe(
    new Set(all).size,
  )
})

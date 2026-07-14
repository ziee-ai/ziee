/**
 * Seeded-surface aggregator + frame.
 *
 * Seeded ENTRIES are now OWNED per-module in `src/modules/<X>/gallery.tsx`
 * (`gallery.seeded`) and auto-discovered by the runtime registry. This file keeps
 * the shared FRAME (real component in an isolated MemoryRouter + a mount-time
 * store seed) + the SAME export surface
 * (`SEEDED_SURFACE_ENTRIES`/`seededSurfaceBySlug`/`SEEDED_SURFACE_SLUGS`).
 */
import { type ReactNode, Suspense, useEffect } from 'react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { Text, Title } from '@ziee/kit'
import { AppErrorBoundary } from '@/components/AppErrorBoundary'
import { Loading } from '@/core/components/Loading'
import { useRunInteraction } from './interactions'
import { SEEDED_SURFACE_ENTRIES } from './support/registry'
import type { SeededSurfaceEntry } from './support/types'

export type { SeededSurfaceEntry }
export { SEEDED_SURFACE_ENTRIES }

export const seededSurfaceBySlug = (slug: string) =>
  SEEDED_SURFACE_ENTRIES.find(e => e.slug === slug)

export const SEEDED_SURFACE_SLUGS = SEEDED_SURFACE_ENTRIES.map(e => e.slug)

const seededTestId = (slug: string) => `gallery-page-${slug}`

/** Renders one seeded-surface entry: the real component + a mount-time store seed. */
export function SeededSurfaceFrame({
  entry,
}: {
  entry: SeededSurfaceEntry
}): ReactNode {
  useEffect(() => {
    void entry.setup?.()
  }, [entry])
  useRunInteraction(entry.interactions, 1200)
  const Component = entry.component
  return (
    <section
      data-testid={seededTestId(entry.slug)}
      data-gallery-state="seeded"
      className="flex flex-col gap-3 border border-border rounded-lg p-4 bg-background"
    >
      <div className="flex flex-col gap-1" data-gallery-chrome>
        <Title level={3}>
          {entry.title}
          <Text tone="muted" className="ml-2 text-sm">
            · seeded
          </Text>
        </Title>
        <Text tone="muted" className="text-sm">
          gallery-page-{entry.slug} · {entry.note}
        </Text>
      </div>
      <div
        data-gallery-frame
        className={
          entry.fullHeight
            ? 'w-full rounded-md border border-border bg-background'
            : 'w-full overflow-hidden rounded-md border border-border bg-background'
        }
        style={entry.fullHeight ? undefined : { height: 720 }}
      >
        <AppErrorBoundary label={`seeded-${entry.slug}`} fallback={() => null}>
          <MemoryRouter initialEntries={[entry.initialPath]}>
            <Routes>
              <Route
                path={entry.path}
                element={
                  <Suspense fallback={<Loading />}>
                    <Component />
                  </Suspense>
                }
              />
            </Routes>
          </MemoryRouter>
        </AppErrorBoundary>
      </div>
    </section>
  )
}

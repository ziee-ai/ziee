/**
 * Page entries for the seeded gallery — each renders a REAL module page inside
 * an isolated `MemoryRouter`, populated through the mock-API cassette.
 *
 * A page frame gives the page a bounded, sized viewport (its `h-full` layouts
 * need a height) and its own router so `useParams`/`useNavigate` and any
 * auto-navigation are isolated per entry (multiple pages share one canvas).
 *
 * Testid convention (matches the story sections):
 *   - each page → `gallery-page-<id>`
 */
import type { ComponentType, ReactNode } from 'react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { Text, Title } from '@/components/ui'
import { AppErrorBoundary } from '@/components/AppErrorBoundary'
import { LlmProviderSettings } from '@/modules/llm-provider/components/LlmProviderSettings'
import { firstEnabledRemoteProviderId } from './fixtures/llm-providers'

export interface GalleryPageEntry {
  /** Stable slug → `gallery-page-<id>`. */
  id: string
  /** Section heading. */
  title: string
  /** One-line note about the state being shown. */
  note?: string
  /** Route pattern the component reads params from (e.g. `/settings/x/:id?`). */
  routePattern: string
  /** Initial URL the MemoryRouter starts at. */
  initialPath: string
  /** The page component under test. */
  component: ComponentType
  /** Frame height in px (default 720). */
  height?: number
}

export const pageTestId = (id: string) => `gallery-page-${id}`

function PageFrame({ entry }: { entry: GalleryPageEntry }): ReactNode {
  const height = entry.height ?? 720
  return (
    <section
      data-testid={pageTestId(entry.id)}
      className="flex flex-col gap-3 border border-border rounded-lg p-4 bg-background"
    >
      <div className="flex flex-col gap-1">
        <Title level={3}>{entry.title}</Title>
        {entry.note ? (
          <Text tone="muted" className="text-sm">
            {entry.note}
          </Text>
        ) : null}
      </div>
      {/* Bounded viewport so the page's h-full layout resolves to a real size. */}
      <div
        className="w-full overflow-hidden rounded-md border border-border bg-background"
        style={{ height }}
      >
        <AppErrorBoundary label={`page-${entry.id}`} fallback={() => null}>
          <MemoryRouter initialEntries={[entry.initialPath]}>
            <Routes>
              <Route path={entry.routePattern} element={<entry.component />} />
            </Routes>
          </MemoryRouter>
        </AppErrorBoundary>
      </div>
    </section>
  )
}

export function GalleryPages({ entries }: { entries: GalleryPageEntry[] }) {
  return (
    <>
      {entries.map(entry => (
        <PageFrame key={entry.id} entry={entry} />
      ))}
    </>
  )
}

/**
 * The page registry. The vertical slice covers the LLM-providers settings page
 * fully populated; the fan-out appends the remaining module pages here.
 */
export const ALL_PAGES: GalleryPageEntry[] = [
  {
    id: 'llm-providers',
    title: 'Settings · LLM Providers (populated)',
    note: 'Real recorded providers + models replayed via mock-API; renders the first provider’s remote settings.',
    routePattern: '/settings/llm-providers/:providerId?',
    // Start on the first enabled remote provider so the populated remote
    // settings render (the local provider's download drawer is exercised by a
    // dedicated local-provider entry in the fan-out).
    initialPath: firstEnabledRemoteProviderId
      ? `/settings/llm-providers/${firstEnabledRemoteProviderId}`
      : '/settings/llm-providers',
    component: LlmProviderSettings,
  },
]

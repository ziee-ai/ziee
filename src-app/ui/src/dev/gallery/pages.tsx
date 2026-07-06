/**
 * Page entries for the seeded gallery — every REAL module route rendered inside
 * an isolated `MemoryRouter`, populated through the mock-API cassette.
 *
 * Pages are ENUMERATED AT RENDER TIME from the router store (populated by
 * `seed()` → `loadModules()`), so every route a module registers is covered
 * automatically — nothing is hand-listed or missed. A page frame gives the page
 * a bounded, sized viewport (its `h-full` layouts need a height) and its own
 * router so `useParams`/`useNavigate` + auto-navigation stay isolated per entry.
 *
 * Testid convention: each page → `gallery-page-<id>`.
 */
import { type ReactNode } from 'react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { Text, Title } from '@/components/ui'
import { AppErrorBoundary } from '@/components/AppErrorBoundary'
import { Loading } from '@/core/components/Loading'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { Suspense, useEffect } from 'react'
import { useRoutesStore } from '@/modules/router/stores'
import type { RouteConfig } from '@/modules/router/types'
import {
  firstEnabledRemoteProviderId,
  llmProvidersList,
} from './fixtures/llm-providers'
import { type OverlayEntry, overlayBySlug } from './overlays'
import { DeepStateFrame, deepStateBySlug } from './deepStates'

export const pageTestId = (id: string) => `gallery-page-${id}`

/**
 * Concrete values for required route params (`:conversationId`, `:projectId`, …)
 * sourced from recorded fixtures. A route whose required param is unresolved is
 * skipped (and surfaced in COVERAGE.md) rather than rendered broken.
 */
// Detail-route params come from recorded fixtures OR the URL (so an isolated
// combo can pin a specific `conversationId` / `projectId` — see the singleton
// isolation policy in SEEDED_GALLERY_PLAN.md). URL wins so each combo is pinned.
function urlParams(): Record<string, string> {
  const q = new URLSearchParams(window.location.search)
  const out: Record<string, string> = {}
  for (const [k, v] of q) out[k] = v
  return out
}

const PARAM_VALUES: Record<string, string | undefined> = {
  providerId: firstEnabledRemoteProviderId ?? llmProvidersList.providers[0]?.id,
  ...urlParams(), // conversationId / projectId / … for isolated detail combos
}

// Routes that are not reviewable page CONTENT (redirects, the gallery itself,
// pure-logic callbacks). Skipped from the page grid.
const SKIP_PATHS = new Set(['/', '/dev/gallery', '/auth/callback'])

interface ResolvedPage {
  id: string
  path: string
  initialPath: string
  element: RouteConfig<any>['element']
}

/**
 * path → stable slug for the testid (`/settings/llm-providers/:x?` →
 * `settings-llm-providers`). Routes with a REQUIRED param get a `-detail` suffix
 * so a swap-type detail route (`/chat/:conversationId`) doesn't collide with its
 * list route (`/chat`) — the two must be distinct enumeration entries.
 */
function slugFor(path: string): string {
  const requiredParam = path.split('/').some(s => s.startsWith(':') && !s.endsWith('?'))
  const cleaned = path
    .replace(/\/:[^/?]+\??/g, '') // drop param segments
    .replace(/^\/+|\/+$/g, '')
    .replace(/\//g, '-')
  const base = cleaned || 'root'
  return requiredParam ? `${base}-detail` : base
}

/** Fill a route path's params; return undefined if a REQUIRED param is unresolved. */
function resolveInitialPath(path: string): string | undefined {
  const segments = path.split('/')
  const out: string[] = []
  for (const seg of segments) {
    if (!seg.startsWith(':')) {
      out.push(seg)
      continue
    }
    const optional = seg.endsWith('?')
    const name = seg.slice(1, optional ? -1 : undefined)
    const value = PARAM_VALUES[name]
    if (value) out.push(value)
    else if (optional) continue // drop the optional segment
    else return undefined // required + unresolved → skip page
  }
  return out.join('/') || '/'
}

/** Build the ordered, de-duplicated page list from the router store. */
export function useResolvedPages(): ResolvedPage[] {
  const routes = useRoutesStore(s => s.routes) as RouteConfig<any>[]
  const seen = new Set<string>()
  const pages: ResolvedPage[] = []
  for (const route of routes) {
    if (!route.path || SKIP_PATHS.has(route.path)) continue
    const initialPath = resolveInitialPath(route.path)
    if (initialPath === undefined) continue
    const id = slugFor(route.path)
    if (seen.has(id)) continue
    seen.add(id)
    if (!route.element) continue
    pages.push({ id, path: route.path, initialPath, element: route.element })
  }
  // Stable, reviewable order: settings pages grouped, then the rest.
  return pages.sort((a, b) => a.id.localeCompare(b.id))
}

function PageFrame({
  page,
  state = 'loaded',
  height = 720,
}: {
  page: ResolvedPage
  state?: string
  height?: number
}): ReactNode {
  return (
    <section
      data-testid={pageTestId(page.id)}
      data-gallery-state={state}
      className="flex flex-col gap-3 border border-border rounded-lg p-4 bg-background"
    >
      <div className="flex flex-col gap-1" data-gallery-chrome>
        <Title level={3}>
          {page.path}
          {state !== 'loaded' ? (
            <Text tone="muted" className="ml-2 text-sm">
              · {state}
            </Text>
          ) : null}
        </Title>
        <Text tone="muted" className="text-sm">
          gallery-page-{page.id} · seeded via mock-API
        </Text>
      </div>
      <div
        className="w-full overflow-hidden rounded-md border border-border bg-background"
        style={{ height }}
      >
        <AppErrorBoundary label={`page-${page.id}`} fallback={() => null}>
          <MemoryRouter initialEntries={[page.initialPath]}>
            <Routes>
              {/* LazyComponentRenderer materializes every route.element form
                  (lazy fn / lazy component / JSX element) — same path the real
                  RouterComponent uses. */}
              <Route
                path={page.path}
                element={
                  <LazyComponentRenderer
                    component={page.element}
                    fallback={<Loading />}
                  />
                }
              />
            </Routes>
          </MemoryRouter>
        </AppErrorBoundary>
      </div>
    </section>
  )
}

/** Renders an overlay in its OPEN state: fires the store open action on mount,
 *  then renders the component (which portals its Sheet/Dialog to the body). */
function OverlayFrame({ entry }: { entry: OverlayEntry }) {
  useEffect(() => {
    entry.open()
  }, [entry])
  const Component = entry.component
  return (
    <section
      data-testid={pageTestId(entry.slug)}
      data-gallery-state="open"
      className="flex flex-col gap-3 border border-border rounded-lg p-4 bg-background"
    >
      <div className="flex flex-col gap-1" data-gallery-chrome>
        <Title level={3}>
          {entry.title}
          <Text tone="muted" className="ml-2 text-sm">
            · open
          </Text>
        </Title>
        <Text tone="muted" className="text-sm">
          gallery-page-{entry.slug} · overlay open-state
        </Text>
      </div>
      <AppErrorBoundary label={`overlay-${entry.slug}`} fallback={() => null}>
        <Suspense fallback={<Loading />}>
          <Component />
        </Suspense>
      </AppErrorBoundary>
    </section>
  )
}

/**
 * Render pages. With no `only`, browses every enumerated page (loaded). With
 * `only=<slug>`, renders just that surface in the given `state` — a page in a
 * data-state, or an overlay (`overlay-*` slug) in its open state.
 */
export function GalleryPages({ only, state }: { only?: string; state?: string }) {
  const pages = useResolvedPages()
  const deep = only ? deepStateBySlug(only) : undefined
  if (deep) return <DeepStateFrame entry={deep} />
  const overlay = only ? overlayBySlug(only) : undefined
  if (overlay) return <OverlayFrame entry={overlay} />
  const shown = only ? pages.filter(p => p.id === only) : pages
  return (
    <>
      {shown.map(page => (
        <PageFrame key={page.id} page={page} state={state} />
      ))}
    </>
  )
}

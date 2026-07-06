/**
 * Seeded-surface entries — real module pages/components rendered with a
 * mount-time STORE SEED that reaches branches the GET-driven data-state pass
 * (empty/error/delayed) structurally cannot.
 *
 * The data-state pass drives the wire (a GET 500 / a latency / an emptied body),
 * so it reaches the `!data` early-returns (load spinner, load-error status). But
 * many branches only render once data is ALREADY LOADED and a *mutation* then
 * fails — e.g. a section's inline "save failed" alert (`data && error`), or a
 * post-load empty derived from seeded state. A GET-only harness never issues the
 * failing mutation, so those arms stay dark.
 *
 * A seeded surface renders the SAME real component inside an isolated
 * `MemoryRouter`, lets it load normally (loaded cassette), then a `setup()` seeds
 * the transient piece through the REAL store (`Store.store.setState(...)`) — the
 * exact channel deepStates/overlays already use. Driven one-per-page-load via
 * `?surface=<slug>` so each seeded singleton store never bleeds across entries.
 */
import {
  type ComponentType,
  type LazyExoticComponent,
  type ReactNode,
  Suspense,
  lazy,
  useEffect,
} from 'react'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { Text, Title } from '@/components/ui'
import { AppErrorBoundary } from '@/components/AppErrorBoundary'
import { Loading } from '@/core/components/Loading'

export interface SeededSurfaceEntry {
  /** Gallery slug → `?surface=<slug>`; also the section testid. */
  slug: string
  /** Human title for the frame. */
  title: string
  /** One-line note about the seeded state this reaches. */
  note: string
  /** Route path the component is mounted under (for useParams/useNavigate). */
  path: string
  /** Concrete initial path (params filled). */
  initialPath: string
  /** The real component to render. */
  component: LazyExoticComponent<ComponentType>
  /** Seed the transient state through the real store (runs after mount). */
  setup?: () => void | Promise<void>
}

const lazyNamed = (loader: () => Promise<any>, name: string) =>
  lazy(() => loader().then(m => ({ default: m[name] })))

/** Compose several named exports into one rendered column (for multi-section pages). */
const lazyCompose = (
  parts: { loader: () => Promise<any>; name: string }[],
): LazyExoticComponent<ComponentType> =>
  lazy(async () => {
    const mods = await Promise.all(parts.map(p => p.loader()))
    const Comps = mods.map((m, i) => m[parts[i].name] as ComponentType)
    return {
      default: () => (
        <div className="flex flex-col gap-4 p-4">
          {Comps.map((C, i) => (
            <C key={i} />
          ))}
        </div>
      ),
    }
  })

const tick = (ms = 80) => new Promise(r => setTimeout(r, ms))

/** Poll until `pred()` is true (store finished its loaded-cassette load), capped. */
export async function whenTrue(pred: () => boolean, tries = 60): Promise<void> {
  for (let i = 0; i < tries; i++) {
    if (pred()) return
    await tick(60)
  }
}

/**
 * Re-apply a store patch a few times over ~1.2s. Stores auto-load on init and
 * some re-subscribe, so a one-shot `setState` seed can be clobbered by a
 * late-arriving load (which resets `error`/`loading`). Re-asserting the patch
 * keeps the seeded branch rendered long enough to be both DOM-visible and — more
 * importantly — counted by the istanbul render pass.
 */
export async function holdPatch(
  apply: () => void,
  times = 10,
  gap = 250,
): Promise<void> {
  for (let i = 0; i < times; i++) {
    apply()
    await tick(gap)
  }
}

export const SEEDED_SURFACE_ENTRIES: SeededSurfaceEntry[] = [
  // ── file_rag admin: 5 section cards share Stores.FileRagAdmin. Once settings
  // load, seeding `.error` flips every section's inline save-error alert. ──────
  {
    slug: 'seeded-file-rag-error',
    title: 'Document RAG admin — save error (all sections)',
    note: 'settings loaded, then Stores.FileRagAdmin.error set → every section inline error alert',
    path: '/settings/file-rag-admin',
    initialPath: '/settings/file-rag-admin',
    component: lazyNamed(
      () => import('@/modules/file-rag/pages/FileRagAdminPage'),
      'FileRagAdminPage',
    ),
    setup: async () => {
      const { FileRagAdmin } = await import(
        '@/modules/file-rag/stores/FileRagAdmin.store'
      )
      await whenTrue(() => FileRagAdmin.store.getState().settings != null)
      await holdPatch(() =>
        FileRagAdmin.store.setState({
          error: 'Failed to save Document RAG settings.',
        } as any),
      )
    },
  },
  // ── code_sandbox resource limits section (behind a non-default tab, so the
  // page pass never mounts it): rendered direct, limits loaded, then error. ────
  {
    slug: 'seeded-sandbox-limits-error',
    title: 'Code Sandbox limits — save error',
    note: 'limits loaded, then Stores.SandboxResourceLimits.error → inline error alert',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/code-sandbox/components/SandboxResourceLimitsSection'),
      'SandboxResourceLimitsSection',
    ),
    setup: async () => {
      const { SandboxResourceLimits } = await import(
        '@/modules/code-sandbox/stores/SandboxResourceLimits.store'
      )
      await whenTrue(() => SandboxResourceLimits.store.getState().limits != null)
      await holdPatch(() =>
        SandboxResourceLimits.store.setState({
          error: 'Failed to save resource limits.',
        } as any),
      )
    },
  },
  // ── code_sandbox resource limits: stuck loading (loading && !limits). ────────
  {
    slug: 'seeded-sandbox-limits-loading',
    title: 'Code Sandbox limits — loading',
    note: 'loading && !limits → the resource-limits load spinner',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/code-sandbox/components/SandboxResourceLimitsSection'),
      'SandboxResourceLimitsSection',
    ),
    setup: async () => {
      const { SandboxResourceLimits } = await import(
        '@/modules/code-sandbox/stores/SandboxResourceLimits.store'
      )
      await holdPatch(() =>
        SandboxResourceLimits.store.setState({ loading: true, limits: null } as any),
      )
    },
  },
  // ── web_search sections (rendered direct): stuck loading (both arms). ────────
  {
    slug: 'seeded-web-search-loading',
    title: 'Web Search settings — loading',
    note: 'loading && !settings / loading && providers.length===0 → both section loaders',
    path: '/',
    initialPath: '/',
    component: lazyCompose([
      {
        loader: () => import('@/modules/web-search/components/WebSearchGlobalSection'),
        name: 'WebSearchGlobalSection',
      },
      {
        loader: () => import('@/modules/web-search/components/WebSearchProvidersSection'),
        name: 'WebSearchProvidersSection',
      },
    ]),
    setup: async () => {
      const { WebSearchAdmin } = await import(
        '@/modules/web-search/stores/WebSearchAdmin.store'
      )
      await holdPatch(() =>
        WebSearchAdmin.store.setState({
          loading: true,
          settings: null,
          providers: [],
        } as any),
      )
    },
  },
  // ── lit_search connectors section: stuck loading (loading && no connectors). ─
  {
    slug: 'seeded-literature-loading',
    title: 'Literature settings — loading',
    note: 'loading && connectors.length===0 → the connectors-section loader',
    path: '/',
    initialPath: '/',
    component: lazyNamed(
      () => import('@/modules/literature/components/settings/LitSearchConnectorsSection'),
      'LitSearchConnectorsSection',
    ),
    setup: async () => {
      const { LitSearchAdmin } = await import(
        '@/modules/literature/stores/LitSearchAdmin.store'
      )
      await holdPatch(() =>
        LitSearchAdmin.store.setState({
          loading: true,
          settings: null,
          connectors: [],
        } as any),
      )
    },
  },
]

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
        className="w-full overflow-hidden rounded-md border border-border bg-background"
        style={{ height: 720 }}
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

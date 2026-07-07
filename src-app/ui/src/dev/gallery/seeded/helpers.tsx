/**
 * Shared helpers for seeded-surface shard files.
 *
 * A "seeded surface" renders a REAL page/component inside an isolated
 * `MemoryRouter`, lets it load normally (loaded cassette), then a `setup()` seeds
 * the transient piece through the REAL store (`Store.store.setState(...)`) — the
 * exact channel deepStates/overlays already use — to reach branches the
 * GET-driven data-state pass (empty/error/delayed) structurally can't (a
 * loaded-then-error alert, a stuck-loading spinner, a seeded-empty list).
 *
 * PARALLEL-GRIND CONTRACT: each shard adds its entries to ITS OWN
 * `seeded/shard<N>.tsx` (exporting `shard<N>Seeded`). The integrator-owned
 * `seededSurfaces.tsx` concatenates them. Shards import ONLY from this file;
 * they never edit `seededSurfaces.tsx`, `overlays.tsx`, `main.tsx`, `pages.tsx`,
 * `stories/index.ts`, `coverage-allowlist.json`, or any generated matrix.
 */
import {
  type ComponentType,
  type LazyExoticComponent,
  lazy,
} from 'react'
import type { InteractionRecipe } from '../interactions'

export interface SeededSurfaceEntry {
  /** Gallery slug → `?surface=<slug>`; also the section testid. Keep it UNIQUE
   *  and shard-prefixed (e.g. `seeded-s3-...`) so shards never collide. */
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
  /** Interaction recipes driven after the seeded surface mounts (click-to-edit
   *  inline forms, expand). Driven via `?surface=<slug>&interact=<name>`. */
  interactions?: InteractionRecipe[]
  /** Render at natural height instead of the fixed 720px overflow-hidden frame.
   *  Use for tall fixtures (e.g. the defect-repro cell suite) where the frame's
   *  own clip would spuriously trip clip-detecting rules (A11/G7) on any cell
   *  below the fold. */
  fullHeight?: boolean
}

/** Lazy-load a named export as the surface component. */
export const lazyNamed = (loader: () => Promise<any>, name: string) =>
  lazy(() => loader().then(m => ({ default: m[name] })))

/** Lazy-load a named export and render it with fixed props (prop-taking components). */
export const lazyProps = (
  loader: () => Promise<any>,
  name: string,
  props: Record<string, unknown>,
): LazyExoticComponent<ComponentType> =>
  lazy(async () => {
    const C = (await loader())[name] as ComponentType<any>
    return { default: () => <C {...props} /> }
  })

/** Compose several named exports into one rendered column (multi-section pages). */
export const lazyCompose = (
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
 * Re-apply a store patch a few times over ~2.5s. Stores auto-load on init and
 * some re-subscribe, so a one-shot `setState` seed can be clobbered by a
 * late-arriving load. Re-asserting the patch keeps the seeded branch rendered
 * long enough to be counted by the istanbul render pass.
 */
export async function holdPatch(
  apply: () => void,
  times = 26,
  gap = 185,
): Promise<void> {
  for (let i = 0; i < times; i++) {
    apply()
    await tick(gap)
  }
}

/**
 * Assert a store patch on a PERMANENT interval (every 150ms, never cleared — dies
 * on page navigation). Use for LOADING arms whose component lazy-mounts at an
 * unpredictable time under the full pass: a fixed-duration `holdPatch` can end
 * before a slow-loading chunk first renders, so the loading arm never commits
 * true. A forever-interval keeps the seed asserting whenever the component
 * finally mounts, making mount-timing irrelevant. (The surface renders one per
 * page-load, so the leaked interval is reclaimed on navigation.)
 */
export function holdForever(apply: () => void): void {
  apply()
  setInterval(apply, 150)
}

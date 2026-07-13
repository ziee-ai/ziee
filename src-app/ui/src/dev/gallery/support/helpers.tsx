/**
 * Shared authoring helpers for per-module `gallery.tsx` files (generalized from
 * the former `seeded/helpers.tsx` + `overlays.tsx` lazy helpers).
 *
 * A seeded surface / overlay renders a REAL page/component inside an isolated
 * frame, lets it load normally (loaded cassette), then a `setup()`/`open()` seeds
 * the transient piece through the REAL store (`Store.store.setState(...)` / the
 * store's open action) — the channel that reaches branches the GET-driven
 * data-state pass (empty/error/delayed) structurally can't.
 */
import { type ComponentType, type LazyExoticComponent, lazy } from 'react'

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

/** Prop-driven overlays whose visibility is a parent-passed `open` prop (not a
 *  store). The overlay analog of `lazyNamed`. Props are cast (dev-only fixtures). */
export const lazyBound = (
  loader: () => Promise<any>,
  name: string,
  props: Record<string, unknown>,
): LazyExoticComponent<ComponentType> =>
  lazy(async () => {
    const C = (await loader())[name] as ComponentType<any>
    return { default: () => <C {...(props as any)} /> }
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
 * unpredictable time under the full pass, where a fixed-duration `holdPatch` can
 * end before the slow-loading chunk first renders.
 */
export function holdForever(apply: () => void): void {
  apply()
  setInterval(apply, 150)
}

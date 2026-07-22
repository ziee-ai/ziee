import { create } from 'zustand'

/**
 * A tiny, ALWAYS-LOADED registry of which always-mounted overlays are currently
 * open, keyed by a stable overlay id.
 *
 * Why: an always-mounted overlay (a global drawer/modal registered in a module's
 * `components: [...]`) decides whether to mount via `shouldMount`. That predicate
 * used to read the drawer's OWN store (`SomeDrawerStore.isOpen`), which forced
 * the module to statically import that store — so registering the module (on
 * login, for an eligible user) pulled the drawer's store onto whatever page you
 * were on, even the chat home. Reading THIS lightweight signal instead lets the
 * drawer's real store stay lazy: it loads only when its opener page/component
 * imports it to call `open()`, which also flips this signal.
 */
interface OverlayVisibilityState {
  open: Record<string, boolean>
  setOpen: (id: string, open: boolean) => void
}

const useOverlayVisibility = create<OverlayVisibilityState>(set => ({
  open: {},
  setOpen: (id, open) =>
    set(s => (!!s.open[id] === open ? s : { open: { ...s.open, [id]: open } })),
}))

/** Reactive: `true` while the overlay `id` is open. Safe inside `shouldMount`. */
export function useOverlayOpen(id: string): boolean {
  return useOverlayVisibility(s => !!s.open[id])
}

/**
 * Flip overlay `id`'s visibility — call from the drawer store's open/close
 * actions (alongside its own `isOpen`), so `shouldMount` mounts/unmounts it
 * without ever needing the store loaded up-front.
 */
export function setOverlayOpen(id: string, open: boolean): void {
  useOverlayVisibility.getState().setOpen(id, open)
}

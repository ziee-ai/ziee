import type { RemoteAccessSet } from '../state'

/** Bracket every API mutation with `saving=true`/`error=null` before,
 *  capture errors into `s.error`, and always settle `saving=false` in `finally`.
 *
 * Accepts a loosely-typed `set` so the store-kit-handed `RemoteAccessSet` works
 * without structural mismatch (mutate only touches `saving`/`error`). */
export default (
  set: RemoteAccessSet,
  body: () => Promise<void>,
) => {
  set((s) => {
    s.saving = true
    s.error = null
  })
  void body().catch((e: unknown) => {
    set((s) => {
      s.error = e instanceof Error ? e.message : 'Failed'
    })
    throw e
  }).finally(() => {
    set((s) => {
      s.saving = false
    })
  })
}

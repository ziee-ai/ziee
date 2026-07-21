/**
 * Bracket every mutation with saving=true/false + error capture. `_`-prefixed so
 * the glob skips it (shared helper, not an action).
 */
export async function mutate(
  // Loosely typed `set` — store-kit hands actions a State-only setter; mutate
  // only touches `saving`/`error`, so a structural draft type is enough.
  set: (recipe: (s: { saving: boolean; error: string | null }) => void) => void,
  body: () => Promise<void>,
) {
  set(s => {
    s.saving = true
    s.error = null
  })
  try {
    await body()
  } catch (e) {
    set(s => {
      s.error = e instanceof Error ? e.message : 'Failed'
    })
    throw e
  } finally {
    set(s => {
      s.saving = false
    })
  }
}

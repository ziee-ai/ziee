/**
 * Canonical LLM-provider display order: the Local provider(s) first, then the
 * rest alphabetically by name. Apply this wherever a provider list is built
 * (admin settings, the chat model picker, per-user provider keys, …) so every
 * surface shows the same order. Returns a new array; does not mutate the input.
 */
export function sortProviders<T extends { provider_type: string; name: string }>(
  list: T[],
): T[] {
  return [...list].sort((a, b) => {
    const aLocal = a.provider_type === 'local'
    const bLocal = b.provider_type === 'local'
    if (aLocal !== bLocal) return aLocal ? -1 : 1
    return a.name.localeCompare(b.name)
  })
}

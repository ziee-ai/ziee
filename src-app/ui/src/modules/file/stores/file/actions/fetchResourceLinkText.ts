import type { FileGet, FileSet } from '../state'

/** Fetch the raw text body at a dynamic same-origin /api/... URL —
 *  used by `useResourceLinkContent` for inline MCP resource_link
 *  blocks whose targets aren't known endpoints in ApiClient.
 *  Attaches the bearer token from the auth store so the request
 *  matches authentication used everywhere else (the previous
 *  inlined `fetch(url)` was unauthenticated). */
export default (_set: FileSet, _get: FileGet) => async (url: string): Promise<string> => {
  // Lazy-import to avoid a circular dep with the api-client module
  // (which itself depends on auth-storage parsing — keeping that
  // out of the file-store load order).
  const { getAuthToken } = await import('@ziee/framework/api-client/core')
  const token = getAuthToken()
  const res = await fetch(url, {
    headers: token ? { Authorization: `Bearer ${token}` } : {},
  })
  if (!res.ok) throw new Error(`HTTP ${res.status}`)
  return await res.text()
}

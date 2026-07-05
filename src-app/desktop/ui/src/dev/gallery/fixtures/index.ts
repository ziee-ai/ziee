/**
 * Desktop gallery cassette — recorded from the same backend, validated against
 * the DESKTOP api-client types + openapi.json. Desktop reuses the web core via
 * the `@/` override plugin, so most surfaces + endpoints overlap; the crawl is
 * filtered to the desktop client's known endpoints.
 */
import type { Cassette } from '../mockApi'
import { authCassette } from './auth'
import { crawlCassette } from './crawl.generated'

export { adminUser, adminMe, adminPermissions } from './auth'

export const GALLERY_CASSETTE: Cassette = {
  ...crawlCassette,
  ...authCassette,
}

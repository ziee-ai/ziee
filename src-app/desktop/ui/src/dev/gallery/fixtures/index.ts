/**
 * Desktop gallery cassette. Layers the per-module seed (shared web modules +
 * desktop-only modules, assembled by `module-seed.ts`) over the recorded crawl
 * BASE, module entries winning. The crawl remains shared infra (recorded from the
 * same backend, validated against the DESKTOP api-client types + openapi.json).
 */
import type { Cassette } from '../mockApi'
import { MODULE_CASSETTE } from '../module-seed'
import { crawlCassette } from './crawl.generated'

export { adminUser, adminMe, adminPermissions } from './auth'

export const GALLERY_CASSETTE: Cassette = {
  ...crawlCassette,
  ...MODULE_CASSETTE,
}

/**
 * Assembles the single cassette the gallery's mock-API replays.
 *
 * Per-module cassette entries are now OWNED in each `src/modules/<X>/gallery.tsx`
 * (`gallery.cassette`) and auto-discovered by the runtime registry
 * (`support/registry.ts` → `MODULE_CASSETTE`). This file layers them over the
 * shared recorded crawl BASE (broad param-less GETs), module entries winning
 * per-key. The crawl remains shared infra (recorded from a real server), not
 * per-module authorship.
 *
 * Also re-exports the admin identity + showcase conversation ids the gallery
 * bootstrap (`seed.ts`) + a couple core files consume — these stay in the shared
 * fixtures DATA layer (referenced across modules).
 */
import type { Cassette } from '../mockApi'
import { MODULE_CASSETTE } from '../support/registry'
import { crawlCassette } from './crawl.generated'

export { adminUser, adminMe, adminPermissions } from './auth'
export { showcaseConversationIds } from './chat'

// Broad crawl first; per-module hand-authored cassettes LAST so they win (they
// carry richer, purpose-seeded data + query/path-param resolvers).
export const GALLERY_CASSETTE: Cassette = {
  ...crawlCassette,
  ...MODULE_CASSETTE,
}

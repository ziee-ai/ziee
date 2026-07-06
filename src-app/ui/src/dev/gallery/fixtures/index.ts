/**
 * Assembles every per-module fixture into the single cassette the gallery's
 * mock-API replays, and re-exports the admin identity used to seed the Auth
 * store. Add a module here as its fixture lands.
 */
import type { Cassette } from '../mockApi'
import { authCassette } from './auth'
import { chatCassette } from './chat'
import { citationsCassette } from './citations'
import { crawlCassette } from './crawl.generated'
import { llmProvidersCassette } from './llm-providers'

export { adminUser, adminMe, adminPermissions } from './auth'
export { showcaseConversationIds } from './chat'

// Broad crawl first; hand-authored per-module fixtures LAST so they win (they
// carry richer, purpose-seeded data + query/path-param resolvers).
export const GALLERY_CASSETTE: Cassette = {
  ...crawlCassette,
  ...authCassette,
  ...llmProvidersCassette,
  ...chatCassette,
  ...citationsCassette,
}

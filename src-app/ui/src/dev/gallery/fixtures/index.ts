/**
 * Assembles every per-module fixture into the single cassette the gallery's
 * mock-API replays, and re-exports the admin identity used to seed the Auth
 * store. Add a module here as its fixture lands.
 */
import type { Cassette } from '../mockApi'
import { authCassette } from './auth'
import { llmProvidersCassette } from './llm-providers'

export { adminUser, adminMe, adminPermissions } from './auth'

export const GALLERY_CASSETTE: Cassette = {
  ...authCassette,
  ...llmProvidersCassette,
}

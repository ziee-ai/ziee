/**
 * Desktop gallery fixtures barrel — the recorded auth seed re-export.
 *
 * The gallery cassette is no longer assembled here: `@ziee/gallery`'s
 * `mountGallery` composes it from `crawlCassette` (the recorded BASE, injected as
 * `GalleryConfig.crawlCassette`) + the per-module seed (shared web modules +
 * desktop-only modules, injected via `GalleryConfig.discoverGalleries` →
 * `module-seed.ts`), module entries winning. The crawl remains shared infra
 * (recorded from the same backend, validated against the DESKTOP api-client types
 * + openapi.json).
 */
export { adminUser, adminMe, adminPermissions } from './auth'

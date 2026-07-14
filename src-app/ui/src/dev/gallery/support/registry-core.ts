/**
 * Re-export of the pure registry logic (now owned by `@ziee/gallery`), kept at
 * this path so the desktop workspace's `module-seed.ts` — which reaches the web
 * registry via the `@/` override fallback — imports `registry-core` unchanged.
 * `import.meta.glob` discovery stays app-side (`registry.ts`); only the pure
 * merge/assert moved to the package.
 */
export type { DiscoveredGallery } from '@ziee/gallery'
export {
  assertUniqueSlugs,
  mergeModuleCassettes,
  moduleNameFromPath,
} from '@ziee/gallery'

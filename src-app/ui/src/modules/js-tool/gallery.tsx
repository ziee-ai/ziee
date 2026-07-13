/**
 * Dev-gallery seed for the `js-tool` module — the `/settings/js-tool`
 * admin page (resource limits for the built-in run_js tool). Auto-discovered by
 * the gallery's runtime registry (`@/dev/gallery/support`); never imported by
 * `module.tsx`, so it is dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'

export const gallery: ModuleGallery = {
  cassette: {
    'JsTool.getSettings': {
      approval_timeout_secs: 120,
      created_at: '2026-01-01T00:00:00.000Z',
      max_concurrent_dispatch: 4,
      max_concurrent_runs: 8,
      max_stack_bytes: 262144,
      max_trace_entries: 256,
      memory_bytes: 67108864,
      updated_at: '2026-01-01T00:00:00.000Z',
      wall_secs: 30,
    },
  },
}

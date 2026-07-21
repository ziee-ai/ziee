/**
 * Dev-gallery seed for the `file-rag` module — the Document RAG admin page's
 * inline save-error state (every section card driven off a single seeded store
 * error). Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyNamed, whenTrue } from '@/dev/gallery/support'

export const gallery: ModuleGallery = {
  seeded: [
    // ── file_rag admin: 5 section cards share FileRagAdmin. Once settings
    // load, seeding `.error` flips every section's inline save-error alert. ──────
    {
      slug: 'seeded-file-rag-error',
      title: 'Document RAG admin — save error (all sections)',
      note: 'settings loaded, then FileRagAdmin.error set → every section inline error alert',
      path: '/settings/file-rag-admin',
      initialPath: '/settings/file-rag-admin',
      component: lazyNamed(
        () => import('@/modules/file-rag/pages/FileRagAdminPage'),
        'FileRagAdminPage',
      ),
      setup: async () => {
        const { FileRagAdminStore } = await import(
          '@/modules/file-rag/stores/fileRagAdmin'
        )
        await whenTrue(() => FileRagAdminStore.getState().settings != null)
        await holdPatch(() =>
          FileRagAdminStore.setState({
            error: 'Failed to save Document RAG settings.',
          } as any),
        )
      },
    },
  ],
}

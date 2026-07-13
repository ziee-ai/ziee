/**
 * Dev-gallery seed for the `citations` module — the import-citations modal and
 * the project-bibliography manage/inline empty surfaces. Owns the shared
 * `citationsCassette`. Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyBound, lazyNamed } from '@/dev/gallery/support'
import { citationsCassette } from '@/dev/gallery/fixtures/citations'

const noop = () => {}

/** A project stub — enough for `Stores.ProjectDetail.project` reads (`project.id`). */
const galleryProject = { id: 'proj-s4', name: 'Gallery Project' }

/** Seed the active project so the project-scoped panels mount past their
 *  `if (!project) return null` guard and their effects fetch with a real id. */
const seedProject = async () => {
  const { ProjectDetail } = await import(
    '@/modules/projects/stores/ProjectDetail.store'
  )
  await holdPatch(() =>
    ProjectDetail.store.setState({ project: galleryProject } as any),
  )
}

export const gallery: ModuleGallery = {
  cassette: citationsCassette,
  overlays: [
    {
      slug: 'overlay-import-citations-modal',
      surface: 'modules/citations/components/ImportCitationsModal',
      title: 'Import citations (modal)',
      component: lazyBound(
        () => import('@/modules/citations/components/ImportCitationsModal'),
        'ImportCitationsModal',
        { open: true, onClose: noop, projectId: null },
      ),
    },
  ],
  seeded: [
    {
      slug: 'seeded-s4-project-bib-manage-empty',
      title: 'Project bibliography manage — empty',
      note: 'initial fetch → loading spinner, then entries.length===0 → <Empty/>',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () =>
          import(
            '@/modules/citations/project-extension/components/ProjectBibliographyManagePanel'
          ),
        'ProjectBibliographyManagePanel',
      ),
      setup: seedProject,
    },
    {
      slug: 'seeded-s4-project-bib-inline-empty',
      title: 'Project bibliography inline — empty',
      note: 'count===0 → the "No references yet — click Manage" link',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () =>
          import(
            '@/modules/citations/project-extension/components/ProjectBibliographyInlinePreview'
          ),
        'ProjectBibliographyInlinePreview',
      ),
      setup: seedProject,
    },
  ],
}

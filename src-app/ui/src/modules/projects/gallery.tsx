/**
 * Dev-gallery seed for the `projects` module — the project form drawer,
 * add-to-project modal, the full-page ProjectDetailPage (loaded/empty/error),
 * and the project-form saving-guard surface. Owns the shared `projectDeepCassette`.
 * Auto-discovered by the gallery's runtime registry (`@/dev/gallery/support`);
 * never imported by `module.tsx`, so it is dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyBound, lazyNamed } from '@/dev/gallery/support'
import { Stores } from '@/core/stores'
import {
  DEEP_PROJECT_ID,
  deepProject,
  deepProjectConversations,
  deepProjectFiles,
  projectDeepCassette,
} from '@/dev/gallery/fixtures/project-deep'

const noop = () => {}

/**
 * Seed the ProjectDetail + ProjectFiles stores for the full-page
 * `ProjectDetailPage` surface. `loadProject` fires on mount (setting loading +
 * loading conversations from the thin cassette); `holdPatch` re-asserts the rich
 * fixture over it so the loaded page renders its populated form.
 */
async function seedProjectDetail(patch: {
  project: typeof deepProject | null
  conversations?: (typeof deepProjectConversations)[number][]
  files?: (typeof deepProjectFiles)[number][]
  error?: string | null
}): Promise<void> {
  const { ProjectDetail } = await import(
    '@/modules/projects/stores/ProjectDetail.store'
  )
  const { ProjectFiles } = await import(
    '@/modules/file/project-extension/stores/ProjectFiles.store'
  )
  await holdPatch(() => {
    ProjectDetail.store.setState({
      project: patch.project,
      loading: false,
      error: patch.error ?? null,
      conversations: patch.conversations ?? [],
      conversationsLoading: false,
      conversationsLoadingMore: false,
      conversationsHasMore: false,
      conversationsError: null,
    } as any)
    ProjectFiles.store.setState({
      currentProjectId: patch.project?.id ?? null,
      files: patch.files ?? [],
      filesLoading: false,
      error: null,
    } as any)
  })
}

export const gallery: ModuleGallery = {
  cassette: projectDeepCassette,
  overlays: [
    {
      slug: 'overlay-project-form-drawer',
      surface: 'modules/projects/components/ProjectFormDrawer',
      title: 'Create project (drawer)',
      component: lazyNamed(
        () => import('@/modules/projects/components/ProjectFormDrawer'),
        'ProjectFormDrawer',
      ),
      open: () => Stores.ProjectDrawer.openProjectDrawer(),
    },
    {
      slug: 'overlay-add-to-project-modal',
      surface: 'modules/projects/components/AddToProjectModal',
      title: 'Add conversation to project (modal)',
      component: lazyBound(
        () => import('@/modules/projects/components/AddToProjectModal'),
        'AddToProjectModal',
        { open: true, conversationId: 'conv-1', onClose: noop },
      ),
    },
  ],
  seeded: [
    {
      slug: 'deep-project-detail',
      title: 'Project detail — loaded (rich)',
      note: 'a fully-populated project: instructions + description + a conversation list + attached knowledge files',
      path: '/projects/:projectId',
      initialPath: `/projects/${DEEP_PROJECT_ID}`,
      component: lazyNamed(
        () => import('@/modules/projects/pages/ProjectDetailPage'),
        'ProjectDetailPage',
      ),
      setup: () =>
        seedProjectDetail({
          project: deepProject,
          conversations: deepProjectConversations,
          files: deepProjectFiles,
        }),
    },
    {
      slug: 'deep-project-detail-empty',
      title: 'Project detail — empty (no chats, no files)',
      note: 'a loaded project with zero conversations + zero knowledge files → the empty affordances',
      path: '/projects/:projectId',
      initialPath: `/projects/${DEEP_PROJECT_ID}`,
      component: lazyNamed(
        () => import('@/modules/projects/pages/ProjectDetailPage'),
        'ProjectDetailPage',
      ),
      setup: () =>
        seedProjectDetail({
          project: { ...deepProject, description: undefined, instructions: undefined },
          conversations: [],
          files: [],
        }),
    },
    {
      slug: 'deep-project-detail-error',
      title: 'Project detail — load error',
      note: 'load settled with no project → the recoverable "Failed to load project" Result',
      path: '/projects/:projectId',
      initialPath: `/projects/${DEEP_PROJECT_ID}`,
      component: lazyNamed(
        () => import('@/modules/projects/pages/ProjectDetailPage'),
        'ProjectDetailPage',
      ),
      setup: () =>
        seedProjectDetail({
          project: null,
          error: 'The project could not be loaded.',
        }),
    },
    {
      slug: 'seeded-s5-project-form-loading',
      title: 'Project form drawer — saving (loading guard)',
      note: 'open && loading → loading render + handleClose `if (loading) return` (ProjectFormDrawer:129)',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/projects/components/ProjectFormDrawer'),
        'ProjectFormDrawer',
      ),
      setup: async () => {
        const { ProjectDrawer } = await import(
          '@/modules/projects/stores/ProjectDrawer.store'
        )
        const seed = () =>
          ProjectDrawer.store.setState({
            open: true,
            loading: true,
            editingProject: null,
          } as any)
        seed()
        // Let the Radix drawer mount its dismissable layer before Escape.
        await new Promise(r => setTimeout(r, 600))
        for (let i = 0; i < 3; i++) {
          document.dispatchEvent(
            new KeyboardEvent('keydown', {
              key: 'Escape',
              code: 'Escape',
              bubbles: true,
            }),
          )
          seed()
          await new Promise(r => setTimeout(r, 250))
        }
        await holdPatch(seed, 6, 250)
      },
    },
  ],
}

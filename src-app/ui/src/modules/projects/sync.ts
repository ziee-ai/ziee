import { registerSync } from '@/core/sync'
import { useProjectsStore } from '@/modules/projects/stores'

// Projects is a small, fully-loaded per-user list (the sidebar widget +
// list page). A full reload is cheap and correct for create / update /
// delete, so the per-surface policy here is simply "reload on any remote
// change" and "reload on (re)connect". (Paginated or infinite-scroll
// surfaces would instead do id-aware update / prepend-on-create.)
registerSync('project', {
  onEvent: () => {
    void useProjectsStore.getState().loadProjects()
  },
  onResync: () => {
    void useProjectsStore.getState().loadProjects()
  },
})

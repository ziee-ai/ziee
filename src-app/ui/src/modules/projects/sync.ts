import { registerSync } from '@/core/sync'
import { useProjectsStore } from '@/modules/projects/stores'

// Projects is a small, fully-loaded per-user list (the sidebar widget +
// list page). A full reload is cheap and correct for create / update /
// delete. NOTE: `loadProjects` early-returns once `isInitialized` is set, so
// reset the flag first to force the refetch (otherwise the handler is a
// permanent no-op after the first load).
const reload = () => {
  useProjectsStore.setState({ isInitialized: false })
  void useProjectsStore.getState().loadProjects()
}

registerSync('project', {
  onEvent: reload,
  onResync: reload,
})

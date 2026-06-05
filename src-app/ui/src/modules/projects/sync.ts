import { Permissions } from '@/api-client/types'
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
  // `projects::read` is Administrators-only (migration 54 — Chat Projects
  // v1 is opt-in, NOT granted to the Users group). Without this gate a
  // non-admin's reconnect `resyncAll` would call `GET /api/projects` → 403
  // (the no-403 E2E gate). onEvent is already owner-scoped server-side.
  requiredPermission: Permissions.ProjectsRead,
})

/**
 * Dev-only gallery wrappers that provide a router context to overlays which call
 * `useNavigate` (e.g. WorkflowDetailDrawer's "Edit" affordance). The real app
 * always renders these inside the app Router; the gallery overlay host (in
 * `@ziee/gallery`, out of tree) does not, so a bare render throws
 * "useNavigate() may be used only in the context of a <Router>". Wrapping the
 * gallery entry in a MemoryRouter is the fix the runtime baseline note itself
 * prescribes ("wrap the modal in a router at its call site"). Tree-shaken from
 * production (only the dev gallery imports this).
 */
import { MemoryRouter } from 'react-router-dom'

import { WorkflowDetailDrawer } from '@/modules/workflow/components/WorkflowDetailDrawer'

export function WorkflowDetailDrawerRouted() {
  return (
    <MemoryRouter>
      <WorkflowDetailDrawer />
    </MemoryRouter>
  )
}

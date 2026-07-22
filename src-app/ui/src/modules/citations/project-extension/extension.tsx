// Registers the citations knowledge kind into the project-extension system, so
// a project's reference list appears as "References" in the project detail
// page's Knowledge area (next to "Knowledge files"). Side-effect import —
// mirrors file/project-extension/extension.tsx. Triggered both by the projects
// auto-discovery glob and a direct import from citations/module.tsx.

import { lazy } from 'react'
import { Book } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { projectExtensionRegistry } from '@/modules/projects/core/extensions'

// Lazy: these panels (+ the ProjectDetail store they pull) load when the
// project Knowledge area renders, not at app boot — the projects auto-discovery
// glob is eager, so a value import here would ride every page's bundle (chat
// home included). The projects registry wraps panels in <Suspense>.
const ProjectBibliographyInlinePreview = lazy(() =>
  import('./components/ProjectBibliographyInlinePreview').then(m => ({
    default: m.ProjectBibliographyInlinePreview,
  })),
)
const ProjectBibliographyManagePanel = lazy(() =>
  import('./components/ProjectBibliographyManagePanel').then(m => ({
    default: m.ProjectBibliographyManagePanel,
  })),
)

projectExtensionRegistry.register({
  name: 'citations',
  slots: {
    knowledge_kinds: {
      label: 'References',
      icon: <Book />,
      inlinePreview: ProjectBibliographyInlinePreview,
      managePanel: ProjectBibliographyManagePanel,
      order: 20,
      // Gate: the "References" project section is backed by
      // `citations::use` (its list/verify endpoints require it — see
      // citations/rest.rs). Without this, a user who has projects but NOT
      // citations::use saw an empty References section that 403s on load
      // (the live leak this audit fixes). `citations::use` is the read gate
      // matching the settings page + route in citations/module.tsx.
      permission: Permissions.CitationsUse,
    },
  },
})

export default {}

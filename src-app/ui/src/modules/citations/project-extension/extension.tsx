// Registers the citations knowledge kind into the project-extension system, so
// a project's reference list appears as "References" in the project detail
// page's Knowledge area (next to "Knowledge files"). Side-effect import —
// mirrors file/project-extension/extension.tsx. Triggered both by the projects
// auto-discovery glob and a direct import from citations/module.tsx.

import { Book } from 'lucide-react'
import { projectExtensionRegistry } from '@/modules/projects/core/extensions'
import { ProjectBibliographyInlinePreview } from './components/ProjectBibliographyInlinePreview'
import { ProjectBibliographyManagePanel } from './components/ProjectBibliographyManagePanel'

projectExtensionRegistry.register({
  name: 'citations',
  slots: {
    knowledge_kinds: {
      label: 'References',
      icon: <Book />,
      inlinePreview: ProjectBibliographyInlinePreview,
      managePanel: ProjectBibliographyManagePanel,
      order: 20,
    },
  },
})

export default {}

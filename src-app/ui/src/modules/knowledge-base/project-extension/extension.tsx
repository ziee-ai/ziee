// Registers the knowledge-base knowledge kind into the project-extension system,
// so a project's attached KBs appear as "Knowledge bases" in the project detail
// page's Knowledge area (next to "Knowledge files" / "References"). Side-effect
// import — mirrors citations/project-extension/extension.tsx. Triggered both by
// the projects auto-discovery glob and a direct import from module.tsx.

import { lazy } from 'react'
import { BookOpen } from 'lucide-react'
import { projectExtensionRegistry } from '@/modules/projects/core/extensions'

// Lazy: these panels (+ the ProjectDetail store they pull) load when the
// project Knowledge area renders, not at app boot — the projects auto-discovery
// glob is eager, so a value import here would ride every page's bundle (chat
// home included). The projects registry wraps panels in <Suspense>.
const ProjectKnowledgeBasesInlinePreview = lazy(() =>
  import('./components/ProjectKnowledgeBasesInlinePreview').then(m => ({
    default: m.ProjectKnowledgeBasesInlinePreview,
  })),
)
const ProjectKnowledgeBasesManagePanel = lazy(() =>
  import('./components/ProjectKnowledgeBasesManagePanel').then(m => ({
    default: m.ProjectKnowledgeBasesManagePanel,
  })),
)

projectExtensionRegistry.register({
  name: 'knowledge-base',
  slots: {
    knowledge_kinds: {
      label: 'Knowledge bases',
      icon: <BookOpen />,
      inlinePreview: ProjectKnowledgeBasesInlinePreview,
      managePanel: ProjectKnowledgeBasesManagePanel,
      order: 30,
    },
  },
})

export default {}

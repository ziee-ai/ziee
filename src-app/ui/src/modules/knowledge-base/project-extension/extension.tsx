// Registers the knowledge-base knowledge kind into the project-extension system,
// so a project's attached KBs appear as "Knowledge bases" in the project detail
// page's Knowledge area (next to "Knowledge files" / "References"). Side-effect
// import — mirrors citations/project-extension/extension.tsx. Triggered both by
// the projects auto-discovery glob and a direct import from module.tsx.

import { BookOpen } from 'lucide-react'
import { projectExtensionRegistry } from '@/modules/projects/core/extensions'
import { ProjectKnowledgeBasesInlinePreview } from './components/ProjectKnowledgeBasesInlinePreview'
import { ProjectKnowledgeBasesManagePanel } from './components/ProjectKnowledgeBasesManagePanel'

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

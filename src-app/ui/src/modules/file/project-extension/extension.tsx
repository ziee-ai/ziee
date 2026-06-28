// Registers the file knowledge kind into the project-extension system.
//
// Side-effect import — running this file is what makes the file kind
// appear in the project detail page's Knowledge area. Two import paths
// trigger it:
//   1. Sibling-module auto-discovery in
//      `modules/projects/extensions/index.ts` via `import.meta.glob`.
//   2. Direct side-effect import from `modules/file/module.tsx` so the
//      registration is bootstrapped whenever the file module loads —
//      independent of the projects module's load order.

import { File } from 'lucide-react'
import { projectExtensionRegistry } from '@/modules/projects/core/extensions'
import { ProjectFilesInlinePreview } from './components/ProjectFilesInlinePreview'
import { ProjectFilesManagePanel } from './components/ProjectFilesManagePanel'

projectExtensionRegistry.register({
  name: 'file',
  slots: {
    knowledge_kinds: {
      label: 'Knowledge files',
      icon: <File />,
      inlinePreview: ProjectFilesInlinePreview,
      managePanel: ProjectFilesManagePanel,
      order: 10,
    },
  },
})

// Default export is required by the glob's import contract — the
// projects auto-discovery treats the file's mere existence as the
// registration trigger; the export is a no-op placeholder.
export default {}

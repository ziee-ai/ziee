// Host-mount project-extension entry point (desktop bundle).
//
// Registers a "Mounted folders" panel into the project detail page's Advanced
// settings slot. Side-effect file: imported by `host-mount/module.tsx` so the
// registration runs at boot. Mirror of the core `mcp/project-extension`.

import { FolderOpenOutlined } from '@ant-design/icons'
import { projectExtensionRegistry } from '@ziee/ui-core/modules/projects/core/extensions'

import { ProjectMountsPanel } from './components/ProjectMountsPanel'

projectExtensionRegistry.register({
  name: 'host-mount',
  slots: {
    advanced_settings: {
      label: 'Mounted folders',
      icon: <FolderOpenOutlined />,
      panel: ProjectMountsPanel,
      // After MCP (10); before any heavier panels.
      order: 20,
    },
  },
})

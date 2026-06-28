// MCP project-extension entry point.
//
// Registers an `advanced_settings` slot contribution that renders the
// project MCP defaults panel inside the project detail page's Advanced
// card. Mirror of `modules/file/project-extension/extension.tsx`.
//
// Side-effect file: imported by `mcp/module.tsx` so the registration
// happens at app boot. Doesn't export a component.

import { Wrench } from 'lucide-react'
import { projectExtensionRegistry } from '@/modules/projects/core/extensions'
import { ProjectMcpSettingsPanel } from './components/ProjectMcpSettingsPanel'

projectExtensionRegistry.register({
  name: 'mcp',
  slots: {
    advanced_settings: {
      label: 'MCP Defaults',
      icon: <Wrench />,
      panel: ProjectMcpSettingsPanel,
      order: 10,
    },
  },
})

import { createModule } from '@/core'
import { lazy } from 'react'
import { useApiKeysStepStore } from './components/ApiKeysStep.store'
import { useMcpServersStepStore } from './components/McpServersStep.store'
import './types'

export default createModule({
  metadata: {
    name: 'guide-getting-started',
    version: '1.0.0',
    description: 'Getting Started guide',
  },
  dependencies: ['onboarding-screen'],
  stores: [
    { name: 'ApiKeysStep', store: useApiKeysStepStore },
    { name: 'McpServersStep', store: useMcpServersStepStore },
  ],
  slots: {
    onboarding: [
      {
        id: 'getting-started',
        title: 'Getting Started',
        description: 'Set up your AI providers and MCP servers to get started.',
        order: 1,
        steps: [
          { id: 'welcome',     title: 'Welcome',      component: lazy(() => import('./components/WelcomeStep')) },
          { id: 'api-keys',    title: 'AI Providers', component: lazy(() => import('./components/ApiKeysStep')) },
          { id: 'mcp-servers', title: 'MCP Servers',  component: lazy(() => import('./components/McpServersStep')) },
          { id: 'finish',      title: 'Finish',       component: lazy(() => import('./components/FinishStep')) },
        ],
      },
    ],
  },
})

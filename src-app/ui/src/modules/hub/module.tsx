import { createModule } from '@/core'

// Hub coordination module
// Sub-modules are auto-discovered from hub/modules/**/module.tsx

export default createModule({
  metadata: {
    name: 'hub',
    version: '1.0.0',
    description: 'Hub catalog coordination module',
  },
  dependencies: [],
})

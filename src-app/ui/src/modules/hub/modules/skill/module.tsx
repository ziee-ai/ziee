import { BookOpen } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { useHubSkillsStore } from '@/modules/hub/modules/skill/stores/hub-skills-store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/hub/modules/skill/types'

const SkillsHubTab = lazyWithPreload(() =>
  import('./components/SkillsHubTab').then(m => ({
    default: m.SkillsHubTab,
  })),
)

export default createModule({
  metadata: {
    name: 'hub-skill',
    version: '1.0.0',
    description: 'Hub catalog for skills',
  },
  // smart-loading gate (build-lifted into the manifest)
  shouldLoad: (ctx) => ctx.isAuthenticated && ctx.can(Permissions.HubModelsRead),
  dependencies: [],
  stores: [{ name: 'HubSkills', store: useHubSkillsStore }],
  slots: {
    hubTabs: [
      {
        id: 'skills',
        label: 'Skills',
        icon: <BookOpen />,
        component: SkillsHubTab,
        order: 40,
        permissions: {
          read: Permissions.SkillsRead,
          refresh: Permissions.HubCatalogManage,
        },
        refresh: async () => {
          await useHubSkillsStore.getState().refresh()
        },
      },
    ],
  },
})

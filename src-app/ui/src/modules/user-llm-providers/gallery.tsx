/**
 * Dev-gallery seed for the `user-llm-providers` module — the per-user
 * provider API-key modal. Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyBound } from '@/dev/gallery/support'
import { llmProvidersList } from '@/dev/gallery/fixtures/llm-providers'

const provider = llmProvidersList.providers[0]
const noop = () => {}

export const gallery: ModuleGallery = {
  overlays: [
    {
      slug: 'overlay-provider-api-key-modal',
      surface: 'modules/user-llm-providers/chat-extension/components/ProviderApiKeyModal',
      title: 'Provider API key (modal)',
      component: lazyBound(
        () =>
          import(
            '@/modules/user-llm-providers/chat-extension/components/ProviderApiKeyModal'
          ),
        'ProviderApiKeyModal',
        {
          providerId: provider.id,
          providerName: (provider as any).name ?? 'OpenAI',
          modelId: 'model-1',
          onSuccess: noop,
          onCancel: noop,
        },
      ),
    },
  ],
}

import { useState, useMemo } from 'react'
import { Button, Select, Tooltip } from '@ziee/kit'
import { TriangleAlert } from 'lucide-react'
import { Stores } from '@ziee/framework/stores'
import type { ProviderWithModels } from '@/api-client/types'
import { newChatModelKey } from '@/modules/user-llm-providers/ModelPicker.store'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { ProviderApiKeyModal } from './ProviderApiKeyModal'

/**
 * ModelSelector Component
 * Self-contained model selection dropdown
 *
 * Features:
 * - Computes available models from providers on-demand
 * - Manages selected model via ModelStore.setModelId()
 * - Shows a warning icon for non-local providers without an API key configured
 * - Prompts for an API key when selecting a model from a non-local provider
 *   with no key (local providers authenticate via a proxy token — never prompt)
 */

/**
 * Whether a provider still needs an API key before its models can be used.
 *
 * Local providers authenticate via an internal, server-minted proxy token —
 * never a user-supplied API key — so they must never show the warning or
 * trigger the key prompt. Every other provider needs a key unless one is
 * already configured (system- or user-level).
 */
function providerNeedsApiKey(
  provider: Pick<ProviderWithModels, 'provider_type' | 'api_key_configured'>,
): boolean {
  return provider.provider_type !== 'local' && !provider.api_key_configured
}

export function ModelSelector() {
  const { selectedByConversation, providers, error, loading } =
    Stores.ModelPicker
  // Key the selection by THIS pane's conversation (resolved via the Stores.Chat
  // bridge → the pane's own conversation in split; the shared new-chat key when
  // there's no conversation yet), so each pane keeps its own model. (ITEM-5)
  const { sending, conversation } = Stores.Chat
  // Per-pane new-chat key (ITEM-37): two new-chat panes keep independent models.
  const pane = useChatPaneOrNull()
  const modelKey = conversation?.id ?? newChatModelKey(pane?.paneId)
  const selectedModelId = selectedByConversation[modelKey]

  const [pendingProviderForKey, setPendingProviderForKey] = useState<{
    providerId: string
    providerName: string
    modelId: string
  } | null>(null)

  const availableModels = useMemo(() => {
    const modelGroups: Array<{
      label: React.ReactNode
      options: Array<{ label: string; value: string; description?: string }>
    }> = []

    providers.forEach(provider => {
      if (provider.llm_models && provider.llm_models.length > 0) {
        const enabledModels = provider.llm_models.filter(model => model.enabled)

        if (enabledModels.length > 0) {
          const label = providerNeedsApiKey(provider) ? (
            <span className="flex items-center gap-1">
              <TriangleAlert className="size-4 shrink-0 text-warning" />
              {provider.name}
            </span>
          ) : (
            provider.name
          )

          modelGroups.push({
            label,
            options: enabledModels.map(model => ({
              label: model.display_name || model.name,
              value: model.id,
              description: model.description,
            })),
          })
        }
      }
    })

    return modelGroups
  }, [providers])

  const handleChange = (value: string | undefined) => {
    if (!value) return
    // Check if selected model belongs to a provider that still needs an API
    // key (local providers never do — they use an internal proxy token).
    for (const provider of providers) {
      if (providerNeedsApiKey(provider) && provider.llm_models) {
        const model = provider.llm_models.find(m => m.id === value)
        if (model) {
          setPendingProviderForKey({
            providerId: provider.id,
            providerName: provider.name,
            modelId: value,
          })
          return
        }
      }
    }
    Stores.ModelPicker.setModelId(modelKey, value)
  }

  const handleKeyProvided = (modelId: string) => {
    setPendingProviderForKey(null)
    Stores.ModelPicker.setModelId(modelKey, modelId)
  }

  // Provider load failed and there's nothing to pick from: a persistent,
  // in-place retry affordance (never a toast that evaporates). Kept compact
  // to fit the composer toolbar; the full ErrorState card is for sections.
  if (error && providers.length === 0) {
    return (
      // `min-w-0` mirrors the loaded branch below: the composer's right toolbar
      // group is shrinkable, so this wrapper must be able to shrink with it
      // rather than overflow into the Send button at narrow widths.
      <div data-testid="model-selector" className="min-w-0">
        <Tooltip content="Couldn't load models. Click to try again.">
          <Button
            variant="ghost"
            icon={<TriangleAlert className="text-destructive" />}
            onClick={() => void Stores.ModelPicker.loadProviders()}
            loading={loading}
            data-testid="ullm-model-retry"
            className="text-[15px] max-w-[200px] text-destructive"
          >
            Models unavailable
          </Button>
        </Tooltip>
      </div>
    )
  }

  return (
    <div data-testid="model-selector" className="min-w-0">
      <Select
        data-testid="ullm-model-select"
        value={selectedModelId ?? undefined}
        onChange={handleChange}
        popupMatchSelectWidth={false}
        placeholder={loading && providers.length === 0 ? 'Loading…' : 'Select Model'}
        aria-label="Model"
        loading={loading && providers.length === 0}
        disabled={sending}
        options={availableModels}
        // Render the SELECTED label as a block-level truncating span. The kit
        // trigger sets both `line-clamp-1` and `flex` on the value slot, and
        // Tailwind orders `display` after `line-clamp`, so `display:flex` wins
        // and `-webkit-line-clamp`'s ellipsis is inert — only its
        // `overflow:hidden` survives, which HARD-CUT a long name into the
        // chevron with no "…". A real block container is what makes
        // `text-overflow: ellipsis` apply (it does not apply to a flex
        // container), and its `overflow:hidden` gives the span a flex
        // auto-minimum-size of 0 so it can actually shrink. Same primitive the
        // kit Menu row uses for its own labels (kit menu.tsx).
        //
        // MUST return undefined when nothing is selected: the kit passes this
        // through as `SelectValue`'s children, and non-null children replace
        // the placeholder.
        labelRender={opt =>
          opt ? <span className="block truncate">{opt.label}</span> : undefined
        }
        // No width override — the kit's own `w-full` is what makes this size to
        // content AND stay shrinkable: the slot wrapper is a flex item whose
        // base size is the label's max-content (so an ordinary name renders IN
        // FULL), and under pressure it shrinks and the label ellipsizes.
        //
        // Do NOT "fix" this to `w-auto`: a <button> is a form control, so
        // `width:auto` is SHRINK-TO-FIT rather than fill-available — it ignores
        // the space actually on offer and simply overflows its container
        // (measured: a 320px trigger inside a 274px composer). That is why the
        // kit sets `w-full` in the first place.
        //
        // `max-w` is the absolute soft ceiling so one pathological name can't
        // swallow a wide toolbar; the composer's right group carries the
        // relative (`60%`) bound that protects the left actions.
        className="text-[15px] min-w-0 max-w-[20rem] border-0 shadow-none bg-transparent"
      />
      {pendingProviderForKey && (
        <ProviderApiKeyModal
          providerId={pendingProviderForKey.providerId}
          providerName={pendingProviderForKey.providerName}
          modelId={pendingProviderForKey.modelId}
          onSuccess={handleKeyProvided}
          onCancel={() => setPendingProviderForKey(null)}
        />
      )}
    </div>
  )
}

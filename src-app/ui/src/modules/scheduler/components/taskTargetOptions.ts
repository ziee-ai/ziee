import type { ProviderWithModels } from '@/api-client/types'
import type { SelectOptionGroup } from '@/components/ui'

/**
 * Grouped model options from the user's accessible providers. Pure + exported so
 * it's unit-testable independent of the store (TEST-5). Mirrors WorkflowRunDialog.
 *
 * Each provider becomes a `{ label, options }` group; each ENABLED model becomes a
 * `{ label: display_name || name, value: id }` option. Providers whose enabled
 * models are empty are dropped (no empty groups in the dropdown).
 */
export function buildModelOptions(
  providers: ProviderWithModels[] | undefined,
): SelectOptionGroup[] {
  return (providers || [])
    .map(p => ({
      label: p.name,
      options: (p.llm_models || [])
        .filter(m => m.enabled)
        .map(m => ({ label: m.display_name || m.name, value: m.id })),
    }))
    .filter(g => g.options.length > 0)
}

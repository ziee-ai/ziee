import { ErrorState, Spin } from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { WebSearchGlobalSection } from './WebSearchGlobalSection'
import { WebSearchProvidersSection } from './WebSearchProvidersSection'

/**
 * Web Search admin settings — one page, two cards: global settings + provider
 * fallback chain, and per-provider config/keys. Mirrors the code-sandbox /
 * memory-admin settings-page layout.
 */
export function WebSearchSettingsPage() {
  // Both loaders write the same `error` field; surface it so a failed initial
  // load isn't silently swallowed (mirrors SandboxResourceLimitsSection).
  const { error, loading, settings } = Stores.WebSearchAdmin

  // Full-page spinner on initial load instead of relying only on section-level
  // spinners (which flash when loading is briefly false before init fires).
  if (loading && !settings) {
    return (
      <SettingsPageContainer
        title="Web Search"
        subtitle="Configure web search + page fetch: the provider fallback chain, API keys, and request caps. Connected-only — fetched/searched content is treated as untrusted data."
      >
        <div className="flex justify-center py-12">
          <Spin size="lg" label="Loading web search settings" />
        </div>
      </SettingsPageContainer>
    )
  }
  // Primary load failed (no settings to show) → replace the sections with a
  // persistent, retryable ErrorState instead of a raw-error banner stacked
  // above empty sections.
  if (error && !settings) {
    return (
      <SettingsPageContainer
        title="Web Search"
        subtitle="Configure web search + page fetch: the provider fallback chain, API keys, and request caps. Connected-only — fetched/searched content is treated as untrusted data."
      >
        <ErrorState
          variant="page"
          resource="web search settings"
          description="The web search settings couldn't be loaded. Check your connection and try again."
          details={error}
          onRetry={() => void Stores.WebSearchAdmin.load()}
          data-testid="websearch-settings-error"
        />
      </SettingsPageContainer>
    )
  }
  return (
    <SettingsPageContainer
      title="Web Search"
      subtitle="Configure web search + page fetch: the provider fallback chain, API keys, and request caps. Connected-only — fetched/searched content is treated as untrusted data."
    >
      <WebSearchGlobalSection />
      <WebSearchProvidersSection />
    </SettingsPageContainer>
  )
}

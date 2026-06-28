import { Alert, Spin } from 'antd'
import { Stores } from '@/core/stores'
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
          <Spin size="large" />
        </div>
      </SettingsPageContainer>
    )
  }
  return (
    <SettingsPageContainer
      title="Web Search"
      subtitle="Configure web search + page fetch: the provider fallback chain, API keys, and request caps. Connected-only — fetched/searched content is treated as untrusted data."
    >
      {error && (
        <Alert
          type="error"
          showIcon
          title="Failed to load web search settings"
          description={error}
          className="mb-3"
        />
      )}
      <WebSearchGlobalSection />
      <WebSearchProvidersSection />
    </SettingsPageContainer>
  )
}

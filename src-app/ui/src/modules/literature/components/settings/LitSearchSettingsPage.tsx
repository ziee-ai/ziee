import { ErrorState, Tabs } from '@ziee/kit'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { LitSearchGlobalSection } from './LitSearchGlobalSection'
import { LitSearchConnectorsSection } from './LitSearchConnectorsSection'
import { LitSearchAdmin } from '@/modules/literature/stores/litSearchAdmin'

/**
 * Literature Search admin settings — one page, two cards: general (enable +
 * completeness + caps) and per-source config. Mirrors the web-search settings
 * page (a thin shell over section components); config lives here because the
 * built-in MCP server row is hidden from the System MCP page.
 */
export function LitSearchSettingsPage() {
  const { error, settings } = LitSearchAdmin
  const subtitle =
    'Search scholarly literature (Europe PMC, Crossref, Semantic Scholar, PubMed, arXiv, CORE), screen results, and fetch open-access full text. Connected-only — results are treated as untrusted data and this is an adjunct to systematic searching.'
  // Primary load failed (no settings) → replace the tabs with a persistent,
  // retryable ErrorState instead of a raw-error banner stacked above them.
  if (error && !settings) {
    return (
      <SettingsPageContainer title="Literature Search" subtitle={subtitle}>
        <ErrorState
          variant="page"
          resource="literature search settings"
          description="The literature search settings couldn't be loaded. Check your connection and try again."
          details={error}
          onRetry={() => void LitSearchAdmin.load()}
          data-testid="lit-settings-error"
        />
      </SettingsPageContainer>
    )
  }
  return (
    <SettingsPageContainer title="Literature Search" subtitle={subtitle}>
      <Tabs
        defaultValue="general"
        data-testid="lit-settings-tabs"
        items={[
          {
            key: 'general',
            label: 'General',
            children: <LitSearchGlobalSection />,
          },
          {
            key: 'sources',
            label: 'Sources',
            children: <LitSearchConnectorsSection />,
          },
        ]}
      />
    </SettingsPageContainer>
  )
}

import { Alert } from 'antd'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { LitSearchGlobalSection } from './LitSearchGlobalSection'
import { LitSearchConnectorsSection } from './LitSearchConnectorsSection'

/**
 * Literature Search admin settings — one page, two cards: general (enable +
 * completeness + caps) and per-source config. Mirrors the web-search settings
 * page (a thin shell over section components); config lives here because the
 * built-in MCP server row is hidden from the System MCP page.
 */
export function LitSearchSettingsPage() {
  const { error } = Stores.LitSearchAdmin
  return (
    <SettingsPageContainer
      title="Literature Search"
      subtitle="Search scholarly literature (Europe PMC, Crossref, Semantic Scholar, PubMed, arXiv, CORE), screen results, and fetch open-access full text. Connected-only — results are treated as untrusted data and this is an adjunct to systematic searching."
    >
      {error && (
        <Alert
          type="error"
          showIcon
          title="Failed to load literature search settings"
          description={error}
          className="mb-3"
        />
      )}
      <LitSearchGlobalSection />
      <LitSearchConnectorsSection />
    </SettingsPageContainer>
  )
}

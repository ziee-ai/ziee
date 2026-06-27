import { Alert, Spin } from 'antd'
import { Stores } from '@/core/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { RebuildStatusSection } from '../components/sections/RebuildStatusSection'
import { MemorySection } from '../components/sections/MemorySection'
import { FullTextSearchSection } from '../components/sections/FullTextSearchSection'
import { SemanticSearchSection } from '../components/sections/SemanticSearchSection'
import { ExtractionSection } from '../components/sections/ExtractionSection'
import { RetentionLimitsSection } from '../components/sections/RetentionLimitsSection'

/**
 * Deployment-wide memory admin settings. One settings-layout page
 * composed of stacked sections, each with its own form so saves are
 * scoped to a single concern (changing the embedding model doesn't
 * also re-PUT the FTS dictionary).
 *
 * Card order:
 *   1. Memory      — master enable + shared `default_top_k`
 *   2. Full-text   — lexical (no model required, so it comes first)
 *   3. Semantic    — vector arm; needs an embedding model
 *   4. Extraction  — which LLM the silent extractor uses
 *   5. Retention   — memory lifetime + extraction quota
 *
 * Summarizer settings (token thresholds + prompts) moved to the
 * `summarization` module — `/settings/summarization-admin`.
 * RebuildStatusSection self-hides unless a rebuild is in flight, so
 * the page is short by default.
 */
export function MemoryAdminPage() {
  const { settings, loading, error } = Stores.MemoryAdmin
  return (
    <SettingsPageContainer
      title="Memory (admin)"
      subtitle="Deployment-wide memory configuration: master toggle, full-text and semantic search, extraction model, retention."
    >
      {/* Surface load failures — the per-section cards render nothing
          until settings arrive, so without this the body is blank on error. */}
      {error && !settings && (
        <Alert
          type="error"
          showIcon
          className="mb-4"
          message="Failed to load memory settings"
          description={error}
        />
      )}
      {loading && !settings && (
        <div className="flex justify-center py-8">
          <Spin />
        </div>
      )}
      <RebuildStatusSection />
      <MemorySection />
      <FullTextSearchSection />
      <SemanticSearchSection />
      <ExtractionSection />
      <RetentionLimitsSection />
    </SettingsPageContainer>
  )
}

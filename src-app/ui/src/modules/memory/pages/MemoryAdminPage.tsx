import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { RebuildStatusSection } from '../components/sections/RebuildStatusSection'
import { EmbeddingEngineSection } from '../components/sections/EmbeddingEngineSection'
import { RetrievalTuningSection } from '../components/sections/RetrievalTuningSection'
import { FullTextSearchSection } from '../components/sections/FullTextSearchSection'
import { RetentionLimitsSection } from '../components/sections/RetentionLimitsSection'
import { SummarizerSection } from '../components/sections/SummarizerSection'

/**
 * Deployment-wide memory admin settings. One settings-layout page
 * composed of stacked sections, each with its own form so saves are
 * scoped to a single concern (changing the embedding model doesn't
 * also re-PUT the summarizer prompts).
 *
 * RebuildStatusSection self-hides unless a rebuild is in flight, so
 * the page is short by default.
 */
export function MemoryAdminPage() {
  return (
    <SettingsPageContainer
      title="Memory (admin)"
      subtitle="Deployment-wide memory configuration: embedding model, retrieval tuning, full-text search, retention, summarizer prompts."
    >
      <RebuildStatusSection />
      <EmbeddingEngineSection />
      <RetrievalTuningSection />
      <FullTextSearchSection />
      <RetentionLimitsSection />
      <SummarizerSection />
    </SettingsPageContainer>
  )
}

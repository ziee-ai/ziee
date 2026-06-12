import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { SummarizationSettingsSection } from '../components/sections/SummarizationSettingsSection'

/**
 * Deployment-wide summarization admin page.
 *
 * Extracted from the memory admin page in migration 91 — summarization
 * is now a standalone module independent of the (privacy-sensitive,
 * opt-in) memory feature, so every deployment can benefit from
 * within-a-conversation context compaction regardless of memory
 * configuration.
 */
export function SummarizationAdminPage() {
  return (
    <SettingsPageContainer
      title="Summarization (admin)"
      subtitle="Deployment-wide conversation summarization: enable, summarizer model (or use the conversation's), token thresholds, and prompt overrides."
    >
      <SummarizationSettingsSection />
    </SettingsPageContainer>
  )
}

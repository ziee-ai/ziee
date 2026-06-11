/**
 * Desktop-only combined Memory settings page.
 *
 * Single-admin desktop deployments want BOTH the user's memory
 * preferences (extraction, retrieval, audit log) AND the deployment-
 * wide admin config (embedding model, retrieval tuning, retention,
 * summarizer) on one screen — there is exactly one user, so the
 * web's user/admin split is just an extra click.
 *
 * Renders the same section components core uses (so behavior stays
 * identical to the web `/settings/memory` + `/settings/admin/memory`
 * pages), separated by a divider. One SettingsPageContainer means
 * one scroll context (the inner `DivScrollY` would fight itself if
 * we stacked two whole pages).
 */

import { Divider, Typography } from 'antd'
import { SettingsPageContainer } from '@ziee/ui-core/modules/settings/components/SettingsPageContainer'

// User sections (web: /settings/memory)
import { PreferencesSection } from '@ziee/ui-core/modules/memory/components/sections/PreferencesSection'
import { MyMemoriesSection } from '@ziee/ui-core/modules/memory/components/sections/MyMemoriesSection'
import { CoreMemorySection } from '@ziee/ui-core/modules/memory/components/sections/CoreMemorySection'
import { AuditLogSection } from '@ziee/ui-core/modules/memory/components/sections/AuditLogSection'

// Admin sections (web: /settings/admin/memory)
import { RebuildStatusSection } from '@ziee/ui-core/modules/memory/components/sections/RebuildStatusSection'
import { MemorySection } from '@ziee/ui-core/modules/memory/components/sections/MemorySection'
import { FullTextSearchSection } from '@ziee/ui-core/modules/memory/components/sections/FullTextSearchSection'
import { SemanticSearchSection } from '@ziee/ui-core/modules/memory/components/sections/SemanticSearchSection'
import { ExtractionSection } from '@ziee/ui-core/modules/memory/components/sections/ExtractionSection'
import { RetentionLimitsSection } from '@ziee/ui-core/modules/memory/components/sections/RetentionLimitsSection'
import { SummarizerSection } from '@ziee/ui-core/modules/memory/components/sections/SummarizerSection'

export function MemoryCombinedPage() {
  return (
    <SettingsPageContainer
      title="Memory"
      subtitle="Persistent memory the assistant keeps about you, plus deployment-wide configuration."
    >
      <Typography.Title level={5} className="!mt-2 !mb-0">
        Your preferences
      </Typography.Title>
      <PreferencesSection />
      <MyMemoriesSection />
      <CoreMemorySection />
      <AuditLogSection />

      <Divider className="!my-4" />

      <Typography.Title level={5} className="!mt-2 !mb-0">
        Administration
      </Typography.Title>
      <RebuildStatusSection />
      <MemorySection />
      <FullTextSearchSection />
      <SemanticSearchSection />
      <ExtractionSection />
      <RetentionLimitsSection />
      <SummarizerSection />
    </SettingsPageContainer>
  )
}

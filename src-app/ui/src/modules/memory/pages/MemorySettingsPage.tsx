import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { PreferencesSection } from '../components/sections/PreferencesSection'
import { MyMemoriesSection } from '../components/sections/MyMemoriesSection'
import { CoreMemorySection } from '../components/sections/CoreMemorySection'
import { AuditLogSection } from '../components/sections/AuditLogSection'

/**
 * Consolidated regular-user memory settings. Replaces the three
 * previous AppLayout routes (`/memories`, `/memories/core-memory`,
 * `/memories/audit-log`) and the standalone `/settings/memory`
 * preferences page with one settings-layout page composed of four
 * stacked sections — matching the codebase convention established
 * by `SandboxSettingsPage`, `HardwareSettings`, etc.
 *
 * Each section gates itself on its specific permission and renders
 * nothing (`return null`) when the viewer lacks read access. The
 * route itself is gated on `anyOf(MemoryRead, CoreMemoryRead)` so a
 * user with either perm can reach the page.
 */
export function MemorySettingsPage() {
  return (
    <SettingsPageContainer
      title="Memory"
      subtitle="Persistent memory the assistant keeps about you across conversations. Both extraction and retrieval are off by default."
    >
      <PreferencesSection />
      <MyMemoriesSection />
      <CoreMemorySection />
      <AuditLogSection />
    </SettingsPageContainer>
  )
}

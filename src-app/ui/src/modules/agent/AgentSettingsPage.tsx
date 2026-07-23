import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { AgentSettingsSection } from './components/AgentSettingsSection'

/**
 * Agent admin settings — deployment-wide agent policy under `/settings/agent`.
 * A single `<Card>` section (sandbox/approval mode + reviewer + budget caps),
 * mirroring the code-sandbox settings surface. Permission gating happens inside
 * the section (it reads the store + renders a permission-denied alert or a
 * read-only form); the page-level container is just a heading + container.
 */
export function AgentSettingsPage() {
  return (
    <SettingsPageContainer
      title="Agent"
      subtitle="Deployment-wide agent policy: sandbox and approval mode for unattended runs, the reviewer agent, and token / step / fan-out budget caps."
    >
      <AgentSettingsSection />
    </SettingsPageContainer>
  )
}

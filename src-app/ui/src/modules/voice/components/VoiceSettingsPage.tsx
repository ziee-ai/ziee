import { Alert, Flex } from '@/components/ui'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { Stores } from '@/core/stores'
import { InstalledVersionsCard } from './InstalledVersionsCard'
import { AvailableVersionsCard } from './AvailableVersionsCard'
import { VoiceConfigCard } from './VoiceConfigCard'
import { ModelCard } from './ModelCard'
import { VoiceInstanceCard } from './VoiceInstanceCard'

/**
 * Voice dictation admin page. Stacks:
 *   1. First-run empty-state banner (enabled but no runtime/model yet)
 *   2. Installed whisper runtimes (set-default / delete)
 *   3. Available runtimes (check-for-updates + install-with-progress)
 *   4. Model readiness (download the ggml model)
 *   5. Instance health (status/state + restart/stop)
 *   6. Deployment-wide config (enable, model, language, timeouts, caps)
 */
export function VoiceSettingsPage() {
  const { versions } = Stores.VoiceRuntimeVersion
  const { status: modelStatus } = Stores.VoiceModel
  const { settings } = Stores.VoiceConfig

  const notReady = versions.length === 0 || !(modelStatus?.present ?? false)
  const showBanner = (settings?.enabled ?? false) && notReady

  return (
    <SettingsPageContainer
      title="Voice Dictation"
      subtitle="Manage the local whisper runtime, model, and deployment-wide voice settings for dictating chat messages."
      data-testid="voice-settings-page-title"
    >
      <Flex className="flex-col gap-3">
        {showBanner && (
          <Alert
            data-testid="voice-not-ready-banner"
            tone="warning"
            title="Voice dictation is enabled but not ready"
            description="Install a whisper runtime and download a model below before users can dictate."
          />
        )}

        <InstalledVersionsCard />
        <AvailableVersionsCard />
        <ModelCard />
        <VoiceInstanceCard />
        <VoiceConfigCard />
      </Flex>
    </SettingsPageContainer>
  )
}

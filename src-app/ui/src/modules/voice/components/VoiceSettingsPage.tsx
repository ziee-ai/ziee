import { Alert, Flex } from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { AvailableModelsCard } from './AvailableModelsCard'
import { AvailableVersionsCard } from './AvailableVersionsCard'
import { InstalledModelsCard } from './InstalledModelsCard'
import { InstalledVersionsCard } from './InstalledVersionsCard'
import { UploadModelDrawer } from './UploadModelDrawer'
import { VoiceConfigCard } from './VoiceConfigCard'
import { VoiceInstanceCard } from './VoiceInstanceCard'

/**
 * Voice dictation admin page. Stacks:
 *   1. First-run empty-state banner (enabled but no runtime/model yet)
 *   2. Installed whisper runtimes (set-default / delete)
 *   3. Available runtimes (check-for-updates + install-with-progress)
 *   4. Model library: installed models + downloadable catalog + upload
 *   5. Instance health (status/state + restart/stop + logs)
 *   6. Deployment-wide config (enable, model, language, timeouts, caps)
 */
export function VoiceSettingsPage() {
  const { versions } = Stores.VoiceRuntimeVersion
  const { installed } = Stores.VoiceModel
  const { settings } = Stores.VoiceConfig

  // Ready when a runtime is installed AND an installed model matches the
  // configured settings.model pointer.
  const modelPresent =
    !!settings?.model && installed.some(m => m.name === settings.model)
  const notReady = versions.length === 0 || !modelPresent
  const showBanner = (settings?.enabled ?? false) && notReady

  return (
    <SettingsPageContainer
      title="Voice Dictation"
      subtitle="Manage the local whisper runtime, models, and deployment-wide voice settings for dictating chat messages."
      data-testid="voice-settings-page-title"
    >
      <Flex className="flex-col gap-3">
        {showBanner && (
          <Alert
            data-testid="voice-not-ready-banner"
            tone="warning"
            title="Voice dictation is enabled but not ready"
            description="Install a whisper runtime and a model below before users can dictate."
          />
        )}

        <InstalledVersionsCard />
        <AvailableVersionsCard />
        <InstalledModelsCard />
        <AvailableModelsCard />
        <VoiceInstanceCard />
        <VoiceConfigCard />
      </Flex>

      <UploadModelDrawer />
    </SettingsPageContainer>
  )
}

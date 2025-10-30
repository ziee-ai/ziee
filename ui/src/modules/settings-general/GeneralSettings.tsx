import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { ThemeSettings } from './components/ThemeSettings'

export default function GeneralSettings() {
  return (
    <SettingsPageContainer title="General">
      <ThemeSettings />
      {/* Future settings cards go here */}
    </SettingsPageContainer>
  )
}

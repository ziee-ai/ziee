import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { AuthProvidersListSection } from './components/AuthProvidersListSection'

/**
 * Admin page for managing third-party auth providers (Google,
 * Microsoft, Apple, generic OIDC/OAuth2). One section; mirrors the
 * code-sandbox page shell pattern.
 */
export function AuthProvidersSettingsPage() {
  return (
    <SettingsPageContainer
      title="Auth providers"
      subtitle="Configure third-party sign-in: Google, Microsoft, Apple, and any OIDC- or OAuth2-compliant identity provider."
    >
      <AuthProvidersListSection />
    </SettingsPageContainer>
  )
}

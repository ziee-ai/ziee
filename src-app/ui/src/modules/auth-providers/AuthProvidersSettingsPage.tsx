import { Alert } from 'antd'
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
      title="Auth Providers"
      subtitle="Configure third-party sign-in: Google, Microsoft, Apple, and any OIDC- or OAuth2-compliant identity provider."
    >
      <Alert
        type="info"
        showIcon
        closable={{ closeIcon: true }}
        className="mb-3"
        title="Configuring auth providers"
        description="Get credentials from the provider's developer console (e.g. Google Cloud Console, Microsoft Entra ID), then enter the client ID and secret below. Enable a provider before testing. Only enable providers you have fully configured."
      />
      <AuthProvidersListSection />
    </SettingsPageContainer>
  )
}

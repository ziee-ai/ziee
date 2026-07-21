import type { StoreProxy } from '@ziee/framework/stores'
import type { useAuthProvidersAdminStore } from './stores/authProvidersAdmin'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    AuthProvidersAdmin: StoreProxy<
      ReturnType<typeof useAuthProvidersAdminStore.getState>
    >
  }
}

/**
 * Templates shown in the "Add provider" dropdown. The template
 * pre-fills standard fields (issuer URLs, default scopes) so the
 * admin only pastes client_id + client_secret.
 */
export interface ProviderTemplate {
  key: string
  label: string
  provider_type: 'oidc' | 'oauth2' | 'apple'
  defaultConfig: Record<string, any>
}

export const PROVIDER_TEMPLATES: ProviderTemplate[] = [
  {
    key: 'google',
    label: 'Google',
    provider_type: 'oidc',
    defaultConfig: {
      client_id: '',
      client_secret: '',
      issuer_url: 'https://accounts.google.com',
      scopes: ['openid', 'email', 'profile'],
      attribute_mapping: {
        user_id: 'sub',
        username: 'email',
        email: 'email',
        display_name: 'name',
        first_name: 'given_name',
        last_name: 'family_name',
      },
      display_name: 'Sign in with Google',
    },
  },
  {
    key: 'microsoft',
    label: 'Microsoft (Entra)',
    provider_type: 'oidc',
    defaultConfig: {
      client_id: '',
      client_secret: '',
      issuer_url: 'https://login.microsoftonline.com/common/v2.0',
      scopes: ['openid', 'email', 'profile'],
      attribute_mapping: {
        user_id: 'sub',
        username: 'preferred_username',
        email: 'email',
        display_name: 'name',
      },
      allowed_tenant_ids: [],
      display_name: 'Sign in with Microsoft',
    },
  },
  {
    key: 'apple',
    label: 'Apple',
    provider_type: 'apple',
    defaultConfig: {
      team_id: '',
      services_id: '',
      key_id: '',
      private_key_path: '',
      scopes: ['name', 'email'],
    },
  },
  {
    key: 'oidc-generic',
    label: 'Generic OIDC (Auth0 / Okta / Authelia / Keycloak)',
    provider_type: 'oidc',
    defaultConfig: {
      client_id: '',
      client_secret: '',
      issuer_url: '',
      scopes: ['openid', 'email', 'profile'],
      attribute_mapping: {
        user_id: 'sub',
        username: 'preferred_username',
        email: 'email',
        display_name: 'name',
      },
    },
  },
  {
    key: 'oauth2-generic',
    label: 'Generic OAuth 2.0 (no OIDC)',
    provider_type: 'oauth2',
    defaultConfig: {
      client_id: '',
      client_secret: '',
      authorization_url: '',
      token_url: '',
      userinfo_url: '',
      scopes: ['email', 'profile'],
      attribute_mapping: {
        user_id: 'sub',
        username: 'username',
        email: 'email',
      },
    },
  },
]

export {}

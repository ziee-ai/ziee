use async_trait::async_trait;
use ldap3::{LdapConnAsync, Scope, SearchEntry};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use super::{AuthError, AuthProvider, AuthProviderTrait, AuthResult, UserAttributes};

/// LDAP provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdapConfig {
    /// LDAP server URL (ldap://hostname:port or ldaps://hostname:port)
    pub url: String,
    /// Base DN for user searches
    pub base_dn: String,
    /// Search filter template (use {username} as placeholder)
    pub search_filter: String,
    /// Bind DN template (use {username} as placeholder), or search then bind
    pub bind_dn_template: Option<String>,
    /// Admin bind DN for search-then-bind flow
    pub admin_bind_dn: Option<String>,
    /// Admin password for search-then-bind flow
    pub admin_password: Option<String>,
    /// Attribute mapping
    pub attribute_mapping: LdapAttributeMapping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LdapAttributeMapping {
    pub username: String,  // Default: "sAMAccountName" or "uid"
    pub email: String,     // Default: "mail"
    pub display_name: Option<String>,  // Default: "displayName"
    pub first_name: Option<String>,    // Default: "givenName"
    pub last_name: Option<String>,     // Default: "sn"
    pub groups: Option<String>,        // Default: "memberOf"
}

impl Default for LdapAttributeMapping {
    fn default() -> Self {
        Self {
            username: "sAMAccountName".to_string(),
            email: "mail".to_string(),
            display_name: Some("displayName".to_string()),
            first_name: Some("givenName".to_string()),
            last_name: Some("sn".to_string()),
            groups: Some("memberOf".to_string()),
        }
    }
}

pub struct LdapAuthProvider {
    name: String,
    config: LdapConfig,
    raw_config: serde_json::Value,
    #[allow(dead_code)]
    pool: PgPool,
}

impl LdapAuthProvider {
    pub fn new(provider: &AuthProvider, pool: PgPool) -> Result<Self, AuthError> {
        let config: LdapConfig = serde_json::from_value(provider.config.clone())
            .map_err(|e| AuthError::ConfigurationError(format!("Invalid LDAP configuration: {}", e)))?;

        Ok(Self {
            name: provider.name.clone(),
            config,
            raw_config: provider.config.clone(),
            pool,
        })
    }

    async fn search_user(&self, username: &str) -> Result<Option<SearchEntry>, AuthError> {
        // Connect to LDAP
        let (conn, mut ldap) = LdapConnAsync::new(&self.config.url)
            .await
            .map_err(|e| AuthError::ConnectionFailed(format!("Failed to connect to LDAP: {}", e)))?;

        ldap3::drive!(conn);

        // Bind as admin if configured
        if let (Some(admin_dn), Some(admin_pw)) = (&self.config.admin_bind_dn, &self.config.admin_password) {
            ldap.simple_bind(admin_dn, admin_pw)
                .await
                .map_err(|e| AuthError::ConfigurationError(format!("Admin bind failed: {}", e)))?;
        }

        // Search for user
        let filter = self.config.search_filter.replace("{username}", username);
        let (rs, _res) = ldap
            .search(
                &self.config.base_dn,
                Scope::Subtree,
                &filter,
                vec!["*"],
            )
            .await
            .map_err(|e| AuthError::InternalError(format!("LDAP search failed: {}", e)))?
            .success()
            .map_err(|e| AuthError::InternalError(format!("LDAP search error: {}", e)))?;

        let _ = ldap.unbind().await;

        // Return first result
        Ok(rs.into_iter().next().map(SearchEntry::construct))
    }
}

#[async_trait]
impl AuthProviderTrait for LdapAuthProvider {
    fn name(&self) -> &str {
        &self.name
    }

    fn provider_type(&self) -> &str {
        "ldap"
    }

    async fn authenticate(
        &self,
        username: &str,
        password: &str,
    ) -> Result<AuthResult, AuthError> {
        // Determine bind DN
        let bind_dn = if let Some(template) = &self.config.bind_dn_template {
            // Direct bind with template
            template.replace("{username}", username)
        } else {
            // Search-then-bind flow
            let user_entry = self.search_user(username)
                .await?
                .ok_or_else(|| AuthError::UserNotFound(format!("User '{}' not found in LDAP", username)))?;

            user_entry.dn
        };

        // Connect and bind as user
        let (conn, mut ldap) = LdapConnAsync::new(&self.config.url)
            .await
            .map_err(|e| AuthError::ConnectionFailed(format!("Failed to connect to LDAP: {}", e)))?;

        ldap3::drive!(conn);

        // Attempt bind with user credentials
        ldap.simple_bind(&bind_dn, password)
            .await
            .map_err(|e| AuthError::InvalidCredentials(format!("LDAP bind failed: {}", e)))?;

        // If bind succeeded, search for user attributes
        let user_entry = self.search_user(username)
            .await?
            .ok_or_else(|| AuthError::UserNotFound(format!("User '{}' not found after authentication", username)))?;

        let _ = ldap.unbind().await;

        // Extract attributes
        let attrs = &user_entry.attrs;
        let get_attr = |name: &str| -> Option<String> {
            attrs.get(name)
                .and_then(|v| v.first())
                .map(|s| s.to_string())
        };

        let username_attr = get_attr(&self.config.attribute_mapping.username)
            .unwrap_or_else(|| username.to_string());
        let email = get_attr(&self.config.attribute_mapping.email)
            .unwrap_or_default();

        let display_name = self.config.attribute_mapping.display_name.as_ref()
            .and_then(|attr| get_attr(attr));
        let first_name = self.config.attribute_mapping.first_name.as_ref()
            .and_then(|attr| get_attr(attr));
        let last_name = self.config.attribute_mapping.last_name.as_ref()
            .and_then(|attr| get_attr(attr));

        // Extract groups
        let groups = self.config.attribute_mapping.groups.as_ref()
            .and_then(|attr| attrs.get(attr))
            .map(|v| v.iter().map(String::from).collect())
            .unwrap_or_default();

        Ok(AuthResult {
            external_id: user_entry.dn.clone(),
            external_username: Some(username_attr.clone()),
            external_email: Some(email.clone()),
            metadata: serde_json::json!({
                "provider": "ldap",
                "dn": user_entry.dn,
                "auth_method": "bind"
            }),
            attributes: UserAttributes {
                username: username_attr,
                email,
                display_name,
                first_name,
                last_name,
                groups,
            },
        })
    }

    async fn test_connection(&self) -> Result<(), AuthError> {
        // Test connection by attempting to connect and bind (if admin credentials provided)
        let (conn, mut ldap) = LdapConnAsync::new(&self.config.url)
            .await
            .map_err(|e| AuthError::ConnectionFailed(format!("Failed to connect to LDAP: {}", e)))?;

        ldap3::drive!(conn);

        if let (Some(admin_dn), Some(admin_pw)) = (&self.config.admin_bind_dn, &self.config.admin_password) {
            ldap.simple_bind(admin_dn, admin_pw)
                .await
                .map_err(|e| AuthError::ConfigurationError(format!("Test bind failed: {}", e)))?;
        }

        let _ = ldap.unbind().await;
        Ok(())
    }

    fn get_config(&self) -> &serde_json::Value {
        &self.raw_config
    }
}

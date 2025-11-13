// User service infrastructure
#![allow(dead_code)]

use std::collections::HashSet;
use uuid::Uuid;

use super::models::{Group, User};
use super::repository::{GroupRepository, UserRepository};
use crate::common::AppError;

// =====================================================
// User Service
// =====================================================

#[derive(Clone)]
pub struct UserService {
    user_repo: UserRepository,
    group_repo: GroupRepository,
}

impl UserService {
    pub fn new(user_repo: UserRepository, group_repo: GroupRepository) -> Self {
        Self {
            user_repo,
            group_repo,
        }
    }

    /// Get user by ID
    pub async fn get_user(&self, id: Uuid) -> Result<Option<User>, AppError> {
        self.user_repo.get_by_id(id).await
    }

    /// Get user by username
    pub async fn get_user_by_username(&self, username: &str) -> Result<Option<User>, AppError> {
        self.user_repo.get_by_username(username).await
    }

    /// Get user permissions (from all groups only, not including direct user permissions)
    pub async fn get_user_permissions(&self, user_id: Uuid) -> Result<HashSet<String>, AppError> {
        let groups = self.user_repo.get_user_groups(user_id).await?;

        let mut permissions = HashSet::new();
        for group in groups {
            permissions.extend(group.permissions.into_iter());
        }

        Ok(permissions)
    }

    /// Get effective permissions (union of user's direct permissions + all group permissions)
    pub async fn get_effective_permissions(&self, user_id: Uuid) -> Result<Vec<String>, AppError> {
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::not_found("User"))?;

        let groups = self.user_repo.get_user_groups(user_id).await?;

        let mut permissions = HashSet::new();

        // Add user's direct permissions
        permissions.extend(user.permissions.into_iter());

        // Add permissions from all active groups
        for group in groups {
            if group.is_active {
                permissions.extend(group.permissions.into_iter());
            }
        }

        // Convert to sorted vector for consistent output
        let mut permissions_vec: Vec<String> = permissions.into_iter().collect();
        permissions_vec.sort();
        Ok(permissions_vec)
    }

    /// Check if user has a specific permission
    pub async fn has_permission(&self, user_id: Uuid, permission: &str) -> Result<bool, AppError> {
        // Get user to check if admin
        let user = self
            .user_repo
            .get_by_id(user_id)
            .await?
            .ok_or_else(|| AppError::not_found("User"))?;

        // Admins have all permissions
        if user.is_admin {
            return Ok(true);
        }

        let permissions = self.get_user_permissions(user_id).await?;

        // Check for wildcard permission
        if permissions.contains("*") {
            return Ok(true);
        }

        // Check exact match
        if permissions.contains(permission) {
            return Ok(true);
        }

        // Check resource wildcard (e.g., "chat:*" matches "chat:read")
        if let Some((resource, _)) = permission.split_once(':') {
            let wildcard = format!("{}:*", resource);
            if permissions.contains(&wildcard) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Assign user to default group
    pub async fn assign_to_default_group(&self, user_id: Uuid) -> Result<(), AppError> {
        // Get default "users" group
        let group = self
            .group_repo
            .get_by_name("users")
            .await?
            .ok_or_else(|| AppError::not_found("Default users group"))?;

        self.user_repo
            .assign_to_group(user_id, group.id, None)
            .await
    }
}

// =====================================================
// Group Service
// =====================================================

#[derive(Clone)]
pub struct GroupService {
    group_repo: GroupRepository,
}

impl GroupService {
    pub fn new(group_repo: GroupRepository) -> Self {
        Self { group_repo }
    }

    /// Get group by ID
    pub async fn get_group(&self, id: Uuid) -> Result<Option<Group>, AppError> {
        self.group_repo.get_by_id(id).await
    }

    /// Get all groups
    pub async fn get_all_groups(&self) -> Result<Vec<Group>, AppError> {
        self.group_repo.get_all().await
    }

    /// Create new group
    pub async fn create_group(
        &self,
        name: &str,
        description: Option<String>,
        permissions: Vec<String>,
    ) -> Result<Group, AppError> {
        self.group_repo.create(name, description, permissions).await
    }

    /// Update group
    pub async fn update_group(
        &self,
        id: Uuid,
        name: Option<String>,
        description: Option<String>,
        permissions: Option<Vec<String>>,
        is_active: Option<bool>,
    ) -> Result<Group, AppError> {
        self.group_repo.update(id, name, description, permissions, is_active).await
    }

    /// Delete group (only non-system groups)
    pub async fn delete_group(&self, id: Uuid) -> Result<(), AppError> {
        self.group_repo.delete(id).await
    }
}

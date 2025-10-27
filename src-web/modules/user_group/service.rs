use super::models::*;
use super::repository::UserGroupRepository;
use uuid::Uuid;

pub struct UserGroupService {
    repository: UserGroupRepository,
}

impl UserGroupService {
    pub fn new(repository: UserGroupRepository) -> Self {
        Self { repository }
    }

    /// Create a new user group
    pub async fn create_group(
        &self,
        request: CreateUserGroupRequest,
    ) -> Result<UserGroup, Box<dyn std::error::Error + Send + Sync>> {
        // Check if group name already exists
        if let Some(_) = self.repository.get_by_name(&request.name).await? {
            return Err("Group name already exists".into());
        }

        Ok(self
            .repository
            .create(
                &request.name,
                request.description.as_deref(),
                request.permissions,
            )
            .await?)
    }

    /// Get group by ID
    pub async fn get_group(
        &self,
        group_id: Uuid,
    ) -> Result<Option<UserGroup>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.repository.get_by_id(group_id).await?)
    }

    /// Update group
    pub async fn update_group(
        &self,
        group_id: Uuid,
        request: UpdateUserGroupRequest,
    ) -> Result<Option<UserGroup>, Box<dyn std::error::Error + Send + Sync>> {
        // Check if group is protected
        if let Some(group) = self.repository.get_by_id(group_id).await? {
            if group.is_protected && request.name.is_some() {
                return Err("Cannot rename protected group".into());
            }
        }

        // If updating name, check if it's already taken
        if let Some(ref name) = request.name {
            if let Some(existing_group) = self.repository.get_by_name(name).await? {
                if existing_group.id != group_id {
                    return Err("Group name already exists".into());
                }
            }
        }

        Ok(self
            .repository
            .update(
                group_id,
                request.name.as_deref(),
                request.description.as_ref().map(|d| Some(d.as_str())),
                request.permissions,
                request.is_active,
            )
            .await?)
    }

    /// Delete group
    pub async fn delete_group(
        &self,
        group_id: Uuid,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        // Check if group is protected
        if let Some(group) = self.repository.get_by_id(group_id).await? {
            if group.is_protected {
                return Err("Cannot delete protected group".into());
            }
        }

        Ok(self.repository.delete(group_id).await?)
    }

    /// List groups with pagination
    pub async fn list_groups(
        &self,
        page: i32,
        per_page: i32,
    ) -> Result<UserGroupListResponse, Box<dyn std::error::Error + Send + Sync>> {
        let (groups, total) = self.repository.list(page, per_page).await?;

        Ok(UserGroupListResponse {
            groups,
            total,
            page,
            per_page,
        })
    }

    /// Assign user to group
    pub async fn assign_user(
        &self,
        group_id: Uuid,
        user_id: Uuid,
        assigned_by: Option<Uuid>,
    ) -> Result<UserGroupMembership, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self
            .repository
            .assign_user(user_id, group_id, assigned_by)
            .await?)
    }

    /// Remove user from group
    pub async fn remove_user(
        &self,
        group_id: Uuid,
        user_id: Uuid,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.repository.remove_user(user_id, group_id).await?)
    }

    /// Get group members
    pub async fn get_group_members(
        &self,
        group_id: Uuid,
    ) -> Result<UserGroupMembersResponse, Box<dyn std::error::Error + Send + Sync>> {
        let memberships = self.repository.get_group_members(group_id).await?;
        let total = memberships.len() as i64;

        Ok(UserGroupMembersResponse {
            memberships,
            total,
        })
    }

    /// Get user's groups
    pub async fn get_user_groups(
        &self,
        user_id: Uuid,
    ) -> Result<Vec<UserGroup>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(self.repository.get_user_groups(user_id).await?)
    }
}

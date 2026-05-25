import { PlusOutlined, TeamOutlined } from '@ant-design/icons'
import { App, Button, Empty, List, Popconfirm, Spin, Tag } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { useEffect, useState } from 'react'

export function UserGroupsDrawer() {
  const { message } = App.useApp()
  const { isOpen, user } = Stores.UserGroupsDrawer
  const { groups } = Stores.UserGroups
  const [userGroupIds, setUserGroupIds] = useState<Set<string>>(new Set())
  const [loadingUserGroups, setLoadingUserGroups] = useState(false)
  const canAssign = usePermission('groups::assign_users')

  // Load user's group memberships when drawer opens
  useEffect(() => {
    if (!isOpen || !user) return

    const loadUserGroups = async () => {
      setLoadingUserGroups(true)
      try {
        // Load members for each group to determine user's memberships
        const membershipPromises = groups.map(async group => {
          try {
            await Stores.UserGroups.loadUserGroupMembers(group.id)
            return { groupId: group.id, isMember: false } // Will update based on members
          } catch {
            return { groupId: group.id, isMember: false }
          }
        })

        await Promise.all(membershipPromises)

        // Check which groups the user belongs to by examining currentGroupMembers
        const userGroups = new Set<string>()
        for (const group of groups) {
          await Stores.UserGroups.loadUserGroupMembers(group.id)
          const members = Stores.UserGroups.__state.currentGroupMembers
          if (members.some(m => m.id === user.id)) {
            userGroups.add(group.id)
          }
        }

        setUserGroupIds(userGroups)
      } catch (error) {
        console.error('Failed to load user group memberships:', error)
      } finally {
        setLoadingUserGroups(false)
      }
    }

    loadUserGroups()
  }, [isOpen, user, groups])

  const handleRemoveFromGroup = async (groupId: string) => {
    if (!user) return

    try {
      await Stores.UserGroups.removeUserFromUserGroup(user.id, groupId)
      message.success('User removed from group successfully')

      // Update user groups list
      setUserGroupIds(prev => {
        const updated = new Set(prev)
        updated.delete(groupId)
        return updated
      })
    } catch (error) {
      console.error('Failed to remove user from group:', error)
      // Error is handled by the store
    }
  }

  const handleAssignToGroup = async (groupId: string) => {
    if (!user) return

    try {
      await Stores.UserGroups.assignUserToUserGroup(user.id, groupId)
      message.success('User assigned to group')
      setUserGroupIds(prev => new Set([...prev, groupId]))
    } catch (error) {
      console.error('Failed to assign:', error)
    }
  }

  return (
    <Drawer
      title={`Groups for ${user?.username}`}
      placement="right"
      onClose={() => {
        Stores.UserGroupsDrawer.closeUserGroupsDrawer()
        setUserGroupIds(new Set())
      }}
      open={isOpen}
      size={400}
      extra={
        canAssign && (
          <Button
            type="text"
            icon={<PlusOutlined aria-hidden="true" />}
            onClick={() => {
              Stores.UserGroupsDrawer.closeUserGroupsDrawer()
              if (user) {
                Stores.AssignGroupDrawer.openAssignGroupDrawer(user)
              }
            }}
            className={'mr-2'}
            aria-label="Assign group"
          />
        )
      }
    >
      {loadingUserGroups ? (
        <div className="flex justify-center py-8">
          <Spin size="large" tip="Loading group memberships..." />
        </div>
      ) : groups.length === 0 ? (
        <Empty description="No groups available" />
      ) : (
        <List
          dataSource={groups}
          renderItem={group => {
            const isMember = userGroupIds.has(group.id)
            const actions = canAssign
              ? [
                  isMember ? (
                    <Popconfirm
                      key="remove"
                      title="Remove user from this group?"
                      onConfirm={() => handleRemoveFromGroup(group.id)}
                      okText="Yes"
                      cancelText="No"
                    >
                      <Button type="link" danger size="small">
                        Remove
                      </Button>
                    </Popconfirm>
                  ) : (
                    <Button
                      key="assign"
                      type="link"
                      size="small"
                      onClick={() => handleAssignToGroup(group.id)}
                    >
                      Assign
                    </Button>
                  ),
                ]
              : []
            return (
              <List.Item actions={actions}>
                <List.Item.Meta
                  avatar={<TeamOutlined />}
                  title={
                    <div className="flex items-center gap-2">
                      {group.name}
                      {isMember && <Tag color="green">Member</Tag>}
                      {group.is_system && <Tag color="orange">System</Tag>}
                    </div>
                  }
                  description={group.description || 'No description'}
                />
              </List.Item>
            )
          }}
        />
      )}
    </Drawer>
  )
}

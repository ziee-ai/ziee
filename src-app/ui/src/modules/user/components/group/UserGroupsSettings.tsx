import { PlusOutlined } from '@ant-design/icons'
import {
  App,
  Button,
  Empty,
  Flex,
  Form,
  Input,
  Pagination,
  Spin,
  Tooltip,
} from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect, useState } from 'react'
import { Stores } from '@/core/stores'
import { Can, usePermission } from '@/core/permissions'
import { Permissions, type CreateGroupRequest, type Group } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer.tsx'
import { EditUserGroupDrawer } from '@/modules/user/components/group/EditUserGroupDrawer.tsx'
import { GroupMembersDrawer } from '@/modules/user/components/group/GroupMembersDrawer.tsx'
import { GroupListItem } from '@/modules/user/components/group/GroupListItem.tsx'

const { TextArea } = Input

// Helper function to validate permissions
const validatePermissions = (_: any, value: string) => {
  if (!value) return Promise.resolve()

  try {
    const parsed = JSON.parse(value)
    if (!Array.isArray(parsed)) {
      return Promise.reject('Must be an array')
    }

    // Check if all values are valid permissions
    const validPermissions = Object.values(Permissions)
    const invalidPermissions = parsed.filter(
      perm => !validPermissions.includes(perm),
    )

    if (invalidPermissions.length > 0) {
      return Promise.reject(
        `Invalid permissions: ${invalidPermissions.join(', ')}`,
      )
    }

    return Promise.resolve()
  } catch {
    return Promise.reject('Invalid JSON format')
  }
}

export function UserGroupsSettings() {
  const { message } = App.useApp()

  const {
    groups,
    total: totalGroups,
    currentPage: storePage,
    pageSize: storePageSize,
    loadingGroups,
    error,
  } = Stores.UserGroups

  const [createModalVisible, setCreateModalVisible] = useState(false)
  const [createForm] = Form.useForm()
  const canCreate = usePermission(Permissions.GroupsCreate)

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.UserGroups.clearError()
    }
  }, [error, message])

  const handleCreateGroup = async (values: any) => {
    try {
      const groupData: CreateGroupRequest = {
        name: values.name,
        description: values.description,
        permissions: values.permissions ? JSON.parse(values.permissions) : [],
      }
      await Stores.UserGroups.createUserGroup(groupData)
      message.success('User group created successfully')
      setCreateModalVisible(false)
      createForm.resetFields()
    } catch (error) {
      console.error('Failed to create user group:', error)
      // Error is handled by the store
    }
  }

  const handleDeleteGroup = async (groupId: string) => {
    try {
      await Stores.UserGroups.deleteUserGroup(groupId)
      message.success('User group deleted successfully')
    } catch (error) {
      console.error('Failed to delete user group:', error)
      // Error is handled by the store
    }
  }

  const handleViewMembers = (group: Group) => {
    Stores.GroupMembersDrawer.openGroupMembersDrawer(group)
  }

  const openEditModal = (group: Group) => {
    Stores.EditUserGroupDrawer.openUserGroupDrawer(group)
  }

  const handlePageChange = (page: number, size?: number) => {
    const newPageSize = size || storePageSize
    const newPage = size && size !== storePageSize ? 1 : page // Reset to page 1 if page size changes

    Stores.UserGroups.loadUserGroups(newPage, newPageSize)
  }

  // Title row with the Add button on the right — matches the
  // pattern used by HardwareSettings (Monitor button) and the
  // other settings pages that hoist their primary action up to
  // the page title.
  const titleWithButton = (
    <div className="flex items-center justify-between w-full">
      <span>User Groups</span>
      <Can permission={Permissions.GroupsCreate}>
        <Tooltip title="Create group">
          <Button
            type="text"
            icon={<PlusOutlined aria-hidden="true" />}
            onClick={() => setCreateModalVisible(true)}
            aria-label="Create group"
          />
        </Tooltip>
      </Can>
    </div>
  )

  return (
    <SettingsPageContainer title={titleWithButton}>
      {loadingGroups ? (
        <div className="flex justify-center py-8">
          <Spin size="large" />
        </div>
      ) : groups.length === 0 ? (
        <Empty description="No user groups yet" />
      ) : (
        // Each GroupListItem already renders its own <Card>, so
        // dropping the outer wrapping card makes every group a
        // direct child of SettingsPageContainer — the container's
        // gap-3 handles the spacing between them.
        <>
          {groups.map(group => (
            <GroupListItem
              key={group.id}
              group={group}
              onEdit={openEditModal}
              onDelete={handleDeleteGroup}
              onViewMembers={handleViewMembers}
            />
          ))}
          <div className="flex justify-end">
            <Pagination
              current={storePage}
              total={totalGroups}
              pageSize={storePageSize}
              showSizeChanger
              showQuickJumper
              showTotal={(total, range) =>
                `${range[0]}-${range[1]} of ${total} groups`
              }
              onChange={handlePageChange}
              onShowSizeChange={handlePageChange}
              pageSizeOptions={['5', '10', '20', '50']}
            />
          </div>
        </>
      )}

      {/* Create Group Modal */}
      <Drawer
        title="Create User Group"
        open={createModalVisible}
        onClose={() => {
          setCreateModalVisible(false)
          createForm.resetFields()
        }}
        footer={null}
        size={600}
        mask={{ closable: false }}
      >
        <Form
          form={createForm}
          layout="vertical"
          onFinish={handleCreateGroup}
          disabled={!canCreate}
        >
          <Form.Item
            name="name"
            label="Group Name"
            rules={[{ required: true, message: 'Please enter group name' }]}
          >
            <Input placeholder="Enter group name" />
          </Form.Item>
          <Form.Item name="description" label="Description">
            <TextArea rows={3} placeholder="Enter group description" />
          </Form.Item>
          <Form.Item
            name="permissions"
            label="Permissions (JSON Array)"
            rules={[{ validator: validatePermissions }]}
          >
            <TextArea
              rows={6}
              placeholder={`["${Permissions.UsersRead}", "${Permissions.UsersEdit}"]`}
            />
          </Form.Item>

          <Form.Item className="mb-0">
            <Flex className="justify-end gap-2">
              <Button
                onClick={() => {
                  setCreateModalVisible(false)
                  createForm.resetFields()
                }}
              >
                {canCreate ? 'Cancel' : 'Close'}
              </Button>
              {canCreate && (
                <Button type="primary" htmlType="submit">
                  Create
                </Button>
              )}
            </Flex>
          </Form.Item>
        </Form>
      </Drawer>

      {/* Edit Group Drawer */}
      <EditUserGroupDrawer />

      {/* Group Members Drawer */}
      <GroupMembersDrawer />
    </SettingsPageContainer>
  )
}

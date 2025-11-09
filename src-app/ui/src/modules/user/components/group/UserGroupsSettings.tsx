import { PlusOutlined } from '@ant-design/icons'
import {
  App,
  Button,
  Card,
  Empty,
  Flex,
  Form,
  Input,
  Pagination,
  Spin,
} from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect, useState } from 'react'
import { Stores } from '@/core/stores'
import type { CreateGroupRequest, Group } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer.tsx'
import { EditUserGroupDrawer } from './EditUserGroupDrawer.tsx'
import { GroupMembersDrawer } from './GroupMembersDrawer.tsx'
import { GroupListItem } from './GroupListItem.tsx'

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
    const invalidPermissions = parsed.filter(perm => !validPermissions.includes(perm))

    if (invalidPermissions.length > 0) {
      return Promise.reject(`Invalid permissions: ${invalidPermissions.join(', ')}`)
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

  return (
    <SettingsPageContainer title="User Groups">
      <div>
        <Card
          title="User Groups"
          extra={
            <Button
              type="text"
              icon={<PlusOutlined aria-hidden="true" />}
              onClick={() => setCreateModalVisible(true)}
              aria-label="Create group"
            />
          }
        >
          {loadingGroups ? (
            <div className="flex justify-center py-8">
              <Spin size="large" />
            </div>
          ) : groups.length === 0 ? (
            <div>
              <Empty description="No user groups found" />
            </div>
          ) : (
            <div className="flex flex-col gap-3">
              {groups.map((group) => (
                <GroupListItem
                  key={group.id}
                  group={group}
                  onEdit={openEditModal}
                  onDelete={handleDeleteGroup}
                  onViewMembers={handleViewMembers}
                />
              ))}
            </div>
          )}

          {groups.length > 0 && (
            <>
              <div className="flex justify-end mt-4">
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
        </Card>

        {/* Create Group Modal */}
        <Drawer
          title="Create User Group"
          open={createModalVisible}
          onClose={() => {
            setCreateModalVisible(false)
            createForm.resetFields()
          }}
          footer={null}
          width={600}
          maskClosable={false}
        >
          <Form
            form={createForm}
            layout="vertical"
            onFinish={handleCreateGroup}
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
                placeholder='["users::read", "users::edit"]'
              />
            </Form.Item>

            <Form.Item className="mb-0">
              <Flex className="gap-2">
                <Button type="primary" htmlType="submit">
                  Create Group
                </Button>
                <Button
                  onClick={() => {
                    setCreateModalVisible(false)
                    createForm.resetFields()
                  }}
                >
                  Cancel
                </Button>
              </Flex>
            </Form.Item>
          </Form>
        </Drawer>

        {/* Edit Group Drawer */}
        <EditUserGroupDrawer />

        {/* Group Members Drawer */}
        <GroupMembersDrawer />
      </div>
    </SettingsPageContainer>
  )
}

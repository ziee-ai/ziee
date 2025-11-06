import {
  EditOutlined,
  LockOutlined,
  PlusOutlined,
  TeamOutlined,
  UserOutlined,
} from '@ant-design/icons'
import {
  App,
  Badge,
  Button,
  Card,
  Descriptions,
  Divider,
  Empty,
  Flex,
  Form,
  Input,
  List,
  Pagination,
  Popconfirm,
  Select,
  Spin,
  Switch,
  Tag,
  Typography,
} from 'antd'
import { Drawer } from '@/components/common/Drawer.tsx'
import { useEffect, useState } from 'react'
import { Stores } from '@/core/stores'
import type { CreateUserRequest, UpdateUserRequest, User } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer.tsx'
import { UserRegistrationSettings } from './UserRegistrationSettings.tsx'

const { Text } = Typography
const { Option } = Select
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

export function UsersSettings() {
  const { message } = App.useApp()

  // Stores
  const {
    users,
    total: totalUsers,
    currentPage: storePage,
    pageSize: storePageSize,
    loading: loadingUsers,
    creating: creatingUser,
    error: usersError,
  } = Stores.Users
  const { groups, error: groupsError } = Stores.UserGroups

  const [createModalVisible, setCreateModalVisible] = useState(false)
  const [editModalVisible, setEditModalVisible] = useState(false)
  const [passwordModalVisible, setPasswordModalVisible] = useState(false)
  const [groupsDrawerVisible, setGroupsDrawerVisible] = useState(false)
  const [assignGroupModalVisible, setAssignGroupModalVisible] = useState(false)
  const [selectedUser, setSelectedUser] = useState<User | null>(null)
  const [createForm] = Form.useForm()
  const [editForm] = Form.useForm()
  const [passwordForm] = Form.useForm()
  const [assignGroupForm] = Form.useForm()
  const [userGroupIds, setUserGroupIds] = useState<Set<string>>(new Set())
  const [loadingUserGroups, setLoadingUserGroups] = useState(false)

  // Show errors
  useEffect(() => {
    if (usersError) {
      message.error(usersError)
      Stores.Users.clearError()
    }
    if (groupsError) {
      message.error(groupsError)
      Stores.UserGroups.clearError()
    }
  }, [usersError, groupsError, message])

  const handleCreateUser = async (values: any) => {
    try {
      const userData: CreateUserRequest = {
        username: values.username,
        email: values.email,
        password: values.password,
        display_name: values.display_name,
        permissions: values.permissions ? JSON.parse(values.permissions) : undefined,
      }

      await Stores.Users.createUser(userData)

      message.success('User created successfully')
      setCreateModalVisible(false)
      createForm.resetFields()
    } catch (error) {
      console.error('Failed to create user:', error)
      // Error is handled by the store
    }
  }

  const handleEditUser = async (values: any) => {
    if (!selectedUser) return

    try {
      const updateData: UpdateUserRequest = {
        username: values.username,
        email: values.email,
        is_active: values.is_active,
        permissions: values.permissions ? JSON.parse(values.permissions) : undefined,
      }

      await Stores.Users.updateUser(selectedUser.id, updateData)

      message.success('User updated successfully')
      setEditModalVisible(false)
      setSelectedUser(null)
      editForm.resetFields()
    } catch (error) {
      console.error('Failed to update user:', error)
      // Error is handled by the store
    }
  }

  const handleResetPassword = async (values: any) => {
    if (!selectedUser) return

    try {
      await Stores.Users.resetUserPassword(selectedUser.id, values.new_password)

      message.success('Password reset successfully')
      setPasswordModalVisible(false)
      setSelectedUser(null)
      passwordForm.resetFields()
    } catch (error) {
      console.error('Failed to reset password:', error)
      // Error is handled by the store
    }
  }

  const handleToggleActive = async (userId: string) => {
    try {
      await Stores.Users.toggleUserActiveStatus(userId)
      message.success('User status updated successfully')
    } catch (error) {
      console.error('Failed to update user status:', error)
      // Error is handled by the store
    }
  }

  const handleAssignGroup = async (values: any) => {
    if (!selectedUser) return

    try {
      await Stores.UserGroups.assignUserToUserGroup(selectedUser.id, values.group_id)
      message.success('User assigned to group successfully')
      setAssignGroupModalVisible(false)
      assignGroupForm.resetFields()

      // Update user groups list
      setUserGroupIds(prev => new Set([...prev, values.group_id]))
    } catch (error) {
      console.error('Failed to assign user to group:', error)
      // Error is handled by the store
    }
  }

  const handleRemoveFromGroup = async (groupId: string) => {
    if (!selectedUser) return

    try {
      await Stores.UserGroups.removeUserFromUserGroup(selectedUser.id, groupId)
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

  const openEditModal = (user: User) => {
    setSelectedUser(user)
    editForm.setFieldsValue({
      username: user.username,
      email: user.email,
      is_active: user.is_active,
      permissions: user.permissions?.length > 0 ? JSON.stringify(user.permissions, null, 2) : '',
    })
    setEditModalVisible(true)
  }

  const openPasswordModal = (user: User) => {
    setSelectedUser(user)
    setPasswordModalVisible(true)
  }

  const openGroupsDrawer = async (user: User) => {
    setSelectedUser(user)
    setGroupsDrawerVisible(true)
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
      // Note: This is a workaround since we don't have a direct API to get user's groups
      const userGroups = new Set<string>()
      for (const group of groups) {
        await Stores.UserGroups.loadUserGroupMembers(group.id)
        const members = Stores.UserGroups.currentGroupMembers
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

  const openAssignGroupModal = (user: User) => {
    setSelectedUser(user)
    setAssignGroupModalVisible(true)
  }

  const getUserActions = (user: User) => {
    const actions: React.ReactNode[] = []

    // Always include the active/inactive status switch first
    actions.push(
      <Popconfirm
        key="active-confirm"
        title={`${user.is_active ? 'Deactivate' : 'Activate'} this user?`}
        onConfirm={() => handleToggleActive(user.id)}
        okText="Yes"
        cancelText="No"
      >
        <Switch className={'!mr-2'} checked={user.is_active} />
      </Popconfirm>,
    )

    actions.push(
      <Button
        key="edit"
        type="text"
        icon={<EditOutlined />}
        onClick={() => openEditModal(user)}
      >
        Edit
      </Button>,
    )

    actions.push(
      <Button
        key="password"
        type="text"
        icon={<LockOutlined />}
        onClick={() => openPasswordModal(user)}
      >
        Reset Password
      </Button>,
    )

    actions.push(
      <Button
        key="groups"
        type="text"
        icon={<TeamOutlined />}
        onClick={() => openGroupsDrawer(user)}
      >
        Groups
      </Button>,
    )

    return actions.filter(Boolean)
  }

  const handlePageChange = (page: number, size?: number) => {
    const newPageSize = size || storePageSize
    const newPage = size && size !== storePageSize ? 1 : page // Reset to page 1 if page size changes

    Stores.Users.loadUsers(newPage, newPageSize)
  }

  return (
    <SettingsPageContainer title="Users">
      <div>
        {/* User Registration Settings */}
        <Flex vertical className="gap-3">
          <UserRegistrationSettings />

          <Card
            title="Users"
            extra={
              <Button
                type="text"
                icon={<PlusOutlined aria-hidden="true" />}
                onClick={() => setCreateModalVisible(true)}
                aria-label="Create user"
              />
            }
          >
            {loadingUsers ? (
              <div className="flex justify-center py-8">
                <Spin size="large" />
              </div>
            ) : users.length === 0 ? (
              <div>
                <Empty description="No users found" />
              </div>
            ) : (
              <div>
                {users.map((user, index) => (
                  <div key={user.id}>
                    <div className="flex items-start gap-3 flex-wrap">
                      {/* User Info */}
                      <div className="flex-1">
                        <div className="flex items-center gap-2 mb-2 flex-wrap">
                          <div className={'flex-1 min-w-48'}>
                            <Flex className="gap-2 items-center">
                              <UserOutlined />
                              <Text className="font-medium">
                                {user.username}
                              </Text>
                              <Badge
                                status={user.is_active ? 'success' : 'error'}
                                text={user.is_active ? 'Active' : 'Inactive'}
                              />
                            </Flex>
                          </div>
                          <div
                            className={'flex gap-1 items-center justify-end'}
                          >
                            {getUserActions(user)}
                          </div>
                        </div>

                        <Descriptions
                          size="small"
                          column={{ xs: 1, sm: 2, md: 3 }}
                          colon={false}
                          styles={{
                            label: { fontSize: '12px', color: '#8c8c8c' },
                            content: { fontSize: '12px' }
                          }}
                        >
                          <Descriptions.Item label="Email">
                            {user.email}
                          </Descriptions.Item>
                          <Descriptions.Item label="Last Login">
                            {user.last_login_at
                              ? new Date(
                                  user.last_login_at,
                                ).toLocaleDateString()
                              : 'Never'}
                          </Descriptions.Item>
                          <Descriptions.Item label="Created">
                            {new Date(user.created_at).toLocaleDateString()}
                          </Descriptions.Item>
                        </Descriptions>
                      </div>
                    </div>
                    {index < users.length - 1 && <Divider className="my-0" />}
                  </div>
                ))}
              </div>
            )}

            {users.length > 0 && (
              <>
                <Divider className="mb-4" />
                <div className="flex justify-end">
                  <Pagination
                    current={storePage}
                    total={totalUsers}
                    pageSize={storePageSize}
                    showSizeChanger
                    showQuickJumper
                    showTotal={(total, range) =>
                      `${range[0]}-${range[1]} of ${total} users`
                    }
                    onChange={handlePageChange}
                    onShowSizeChange={handlePageChange}
                    pageSizeOptions={['5', '10', '20', '50']}
                  />
                </div>
              </>
            )}
          </Card>
        </Flex>

        {/* Create User Modal */}
        <Drawer
          title="Create User"
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
            onFinish={handleCreateUser}
          >
            <Form.Item
              name="username"
              label="Username"
              rules={[{ required: true, message: 'Please enter username' }]}
            >
              <Input placeholder="Enter username" />
            </Form.Item>
            <Form.Item
              name="email"
              label="Email"
              rules={[
                {
                  required: true,
                  type: 'email',
                  message: 'Please enter valid email',
                },
              ]}
            >
              <Input placeholder="Enter email" />
            </Form.Item>
            <Form.Item
              name="password"
              label="Password"
              rules={[
                { required: true, message: 'Please enter password' },
                { min: 6, message: 'Password must be at least 6 characters' },
              ]}
            >
              <Input.Password placeholder="Enter password" />
            </Form.Item>
            <Form.Item
              name="display_name"
              label="Display Name"
            >
              <Input placeholder="Enter display name (optional)" />
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
                <Button type="primary" htmlType="submit" loading={creatingUser}>
                  Create User
                </Button>
                <Button
                  onClick={() => {
                    setCreateModalVisible(false)
                    createForm.resetFields()
                  }}
                  disabled={creatingUser}
                >
                  Cancel
                </Button>
              </Flex>
            </Form.Item>
          </Form>
        </Drawer>

        {/* Edit User Modal */}
        <Drawer
          title="Edit User"
          open={editModalVisible}
          onClose={() => {
            setEditModalVisible(false)
            setSelectedUser(null)
            editForm.resetFields()
          }}
          footer={null}
          width={600}
          maskClosable={false}
        >
          <Form form={editForm} layout="vertical" onFinish={handleEditUser}>
            <Form.Item
              name="username"
              label="Username"
              rules={[{ required: true, message: 'Please enter username' }]}
            >
              <Input placeholder="Enter username" />
            </Form.Item>
            <Form.Item
              name="email"
              label="Email"
              rules={[
                {
                  required: true,
                  type: 'email',
                  message: 'Please enter valid email',
                },
              ]}
            >
              <Input placeholder="Enter email" />
            </Form.Item>
            <Form.Item
              name="is_active"
              label="Active"
              valuePropName="checked"
            >
              <Switch />
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
                  Update User
                </Button>
                <Button
                  onClick={() => {
                    setEditModalVisible(false)
                    setSelectedUser(null)
                    editForm.resetFields()
                  }}
                >
                  Cancel
                </Button>
              </Flex>
            </Form.Item>
          </Form>
        </Drawer>

        {/* Reset Password Modal */}
        <Drawer
          title="Reset Password"
          open={passwordModalVisible}
          onClose={() => {
            setPasswordModalVisible(false)
            setSelectedUser(null)
            passwordForm.resetFields()
          }}
          footer={null}
          maskClosable={false}
        >
          <Form
            form={passwordForm}
            layout="vertical"
            onFinish={handleResetPassword}
          >
            <Form.Item
              name="new_password"
              label="New Password"
              rules={[
                { required: true, message: 'Please enter new password' },
                { min: 6, message: 'Password must be at least 6 characters' },
              ]}
            >
              <Input.Password
                placeholder="Enter new password"
              />
            </Form.Item>
            <Form.Item
              name="confirm_password"
              label="Confirm Password"
              dependencies={['new_password']}
              rules={[
                { required: true, message: 'Please confirm password' },
                ({ getFieldValue }) => ({
                  validator(_, value) {
                    if (!value || getFieldValue('new_password') === value) {
                      return Promise.resolve()
                    }
                    return Promise.reject('Passwords do not match')
                  },
                }),
              ]}
            >
              <Input.Password
                placeholder="Confirm new password"
              />
            </Form.Item>
            <Form.Item className="mb-0">
              <Flex className="gap-2">
                <Button type="primary" htmlType="submit">
                  Reset Password
                </Button>
                <Button
                  onClick={() => {
                    setPasswordModalVisible(false)
                    setSelectedUser(null)
                    passwordForm.resetFields()
                  }}
                >
                  Cancel
                </Button>
              </Flex>
            </Form.Item>
          </Form>
        </Drawer>

        {/* Groups Drawer */}
        <Drawer
          title={`Groups for ${selectedUser?.username}`}
          placement="right"
          onClose={() => {
            setGroupsDrawerVisible(false)
            setUserGroupIds(new Set())
          }}
          open={groupsDrawerVisible}
          width={400}
          extra={
            <Button
              type="text"
              icon={<PlusOutlined aria-hidden="true" />}
              onClick={() => {
                setGroupsDrawerVisible(false)
                openAssignGroupModal(selectedUser!)
              }}
              className={'mr-2'}
              aria-label="Assign group"
            />
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
                return (
                  <List.Item
                    actions={[
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
                          onClick={async () => {
                            try {
                              await Stores.UserGroups.assignUserToUserGroup(
                                selectedUser!.id,
                                group.id,
                              )
                              message.success('User assigned to group')
                              setUserGroupIds(
                                prev => new Set([...prev, group.id]),
                              )
                            } catch (error) {
                              console.error('Failed to assign:', error)
                            }
                          }}
                        >
                          Assign
                        </Button>
                      ),
                    ]}
                  >
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

        {/* Assign Group Modal */}
        <Drawer
          title="Assign to Group"
          open={assignGroupModalVisible}
          onClose={() => {
            setAssignGroupModalVisible(false)
            setSelectedUser(null)
            assignGroupForm.resetFields()
          }}
          footer={null}
          maskClosable={false}
        >
          <Form
            form={assignGroupForm}
            layout="vertical"
            onFinish={handleAssignGroup}
          >
            <Form.Item
              name="group_id"
              label="Select Group"
              rules={[
                {
                  required: true,
                  message: 'Please select a group',
                },
              ]}
            >
              <Select placeholder="Select group to assign">
                {groups.map(group => (
                  <Option key={group.id} value={group.id}>
                    {group.name}
                  </Option>
                ))}
              </Select>
            </Form.Item>
            <Form.Item className="mb-0">
              <Flex className="gap-2">
                <Button type="primary" htmlType="submit">
                  Assign Group
                </Button>
                <Button
                  onClick={() => {
                    setAssignGroupModalVisible(false)
                    setSelectedUser(null)
                    assignGroupForm.resetFields()
                  }}
                >
                  Cancel
                </Button>
              </Flex>
            </Form.Item>
          </Form>
        </Drawer>
      </div>
    </SettingsPageContainer>
  )
}

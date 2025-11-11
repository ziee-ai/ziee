import {
  DeleteOutlined,
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
  Pagination,
  Popconfirm,
  Spin,
  Switch,
  Typography,
} from 'antd'
import { useEffect } from 'react'
import { Stores } from '@/core/stores'
import type { User } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer.tsx'
import { UserRegistrationSettings } from './UserRegistrationSettings.tsx'
import { CreateUserDrawer } from './CreateUserDrawer.tsx'
import { EditUserDrawer } from './EditUserDrawer.tsx'
import { ResetPasswordDrawer } from './ResetPasswordDrawer.tsx'
import { UserGroupsDrawer } from './UserGroupsDrawer.tsx'
import { AssignGroupDrawer } from './AssignGroupDrawer.tsx'

const { Text } = Typography

export function UsersSettings() {
  const { message } = App.useApp()

  // Stores
  const {
    users,
    total: totalUsers,
    currentPage: storePage,
    pageSize: storePageSize,
    loading: loadingUsers,
    error: usersError,
  } = Stores.Users
  const { error: groupsError } = Stores.UserGroups

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

  const handleToggleActive = async (userId: string) => {
    try {
      await Stores.Users.toggleUserActiveStatus(userId)
      message.success('User status updated successfully')
    } catch (error) {
      console.error('Failed to update user status:', error)
      // Error is handled by the store
    }
  }

  const handleDelete = async (userId: string) => {
    try {
      await Stores.Users.deleteUser(userId)
      message.success('User deleted successfully')
    } catch (error) {
      console.error('Failed to delete user:', error)
      // Error is handled by the store
    }
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
        <Switch className={'mr-2!'} checked={user.is_active} />
      </Popconfirm>,
    )

    actions.push(
      <Button
        key="edit"
        type="text"
        icon={<EditOutlined />}
        onClick={() => Stores.EditUserDrawer.openEditUserDrawer(user)}
      >
        Edit
      </Button>,
    )

    actions.push(
      <Button
        key="password"
        type="text"
        icon={<LockOutlined />}
        onClick={() => Stores.ResetPasswordDrawer.openResetPasswordDrawer(user)}
      >
        Reset Password
      </Button>,
    )

    actions.push(
      <Button
        key="groups"
        type="text"
        icon={<TeamOutlined />}
        onClick={() => Stores.UserGroupsDrawer.openUserGroupsDrawer(user)}
      >
        Groups
      </Button>,
    )

    actions.push(
      <Popconfirm
        key="delete"
        title="Are you sure you want to delete this user?"
        onConfirm={() => handleDelete(user.id)}
        okText="Yes"
        cancelText="No"
      >
        <Button
          type="text"
          danger
          icon={<DeleteOutlined aria-hidden="true" />}
          aria-label={`Delete ${user.username}`}
        >
          Delete
        </Button>
      </Popconfirm>,
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
                onClick={() => Stores.CreateUserDrawer.openCreateUserDrawer()}
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

        {/* Drawer Components */}
        <CreateUserDrawer />
        <EditUserDrawer />
        <ResetPasswordDrawer />
        <UserGroupsDrawer />
        <AssignGroupDrawer />
      </div>
    </SettingsPageContainer>
  )
}

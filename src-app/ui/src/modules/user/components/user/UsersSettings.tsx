import { Trash2, Pencil, Lock, Plus, Users, User as UserIcon } from 'lucide-react'
import {
  Badge,
  Button,
  Card,
  Switch,
  Descriptions,
  Empty,
  Flex,
  Tooltip,
  message,
  Separator,
  Text,
  Confirm,
  Pagination,
} from '@/components/ui'
import { Loading } from '@/core/components/Loading'
import { useEffect } from 'react'
import { Stores } from '@/core/stores'
import { Can, usePermission } from '@/core/permissions'
import { Permissions, type User } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer.tsx'
// UserRegistrationSettings — temporarily un-imported; see comment in JSX.
// import { UserRegistrationSettings } from '@/modules/user/components/user/UserRegistrationSettings.tsx'
import { CreateUserDrawer } from '@/modules/user/components/user/CreateUserDrawer.tsx'
import { EditUserDrawer } from '@/modules/user/components/user/EditUserDrawer.tsx'
import { ResetPasswordDrawer } from '@/modules/user/components/user/ResetPasswordDrawer.tsx'
import { UserGroupsDrawer } from '@/modules/user/components/user/UserGroupsDrawer.tsx'
import { AssignGroupDrawer } from '@/modules/user/components/user/AssignGroupDrawer.tsx'

export function UsersSettings() {
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
  const { user: currentUser } = Stores.Auth

  const canEdit = usePermission(Permissions.UsersEdit)
  const canResetPassword = usePermission(Permissions.UsersResetPassword)
  const canAssignGroups = usePermission(Permissions.GroupsAssignUsers)
  const canDelete = usePermission(Permissions.UsersDelete)
  const canToggleStatus = usePermission(Permissions.UsersToggleStatus)

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
  }, [usersError, groupsError])

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

    // Self / root-admin lockout guards: hide destructive controls on
    // the viewer's own row and on the root admin row regardless of
    // permission (the backend enforces these too, but the UI should
    // never offer a button that will reliably 400/403).
    const isSelf = currentUser?.id === user.id
    const isRootAdmin = user.is_admin

    // Active/inactive switch
    if (canToggleStatus && !isSelf && !isRootAdmin) {
      actions.push(
        <div key="active-confirm" className="inline-flex items-center">
          <Switch className={'mr-2!'} checked={user.is_active} />
          <Confirm
            title={`${user.is_active ? 'Deactivate' : 'Activate'} this user?`}
            onConfirm={() => handleToggleActive(user.id)}
            okText="OK"
            cancelText="Cancel"
          >
            <Button variant="ghost" size="sm">
              {user.is_active ? 'Deactivate' : 'Activate'}
            </Button>
          </Confirm>
        </div>,
      )
    }

    if (canEdit) {
      actions.push(
        <Button
          key="edit"
          variant="ghost"
          icon={<Pencil />}
          onClick={() => Stores.EditUserDrawer.openEditUserDrawer(user)}
        >
          Edit
        </Button>,
      )
    }

    if (canResetPassword) {
      actions.push(
        <Button
          key="password"
          variant="ghost"
          icon={<Lock />}
          onClick={() =>
            Stores.ResetPasswordDrawer.openResetPasswordDrawer(user)
          }
        >
          Reset Password
        </Button>,
      )
    }

    if (canAssignGroups) {
      actions.push(
        <Button
          key="groups"
          variant="ghost"
          icon={<Users />}
          onClick={() => Stores.UserGroupsDrawer.openUserGroupsDrawer(user)}
        >
          Groups
        </Button>,
      )
    }

    if (canDelete && !isSelf && !isRootAdmin) {
      actions.push(
        <div key="delete" className="inline-flex items-center">
          <Confirm
            title="Are you sure you want to delete this user?"
            onConfirm={() => handleDelete(user.id)}
            okText="OK"
            cancelText="Cancel"
          >
            <Button
              variant="destructive"
              icon={<Trash2 aria-hidden="true" />}
              aria-label={`Delete ${user.username}`}
            >
              Delete
            </Button>
          </Confirm>
        </div>,
      )
    }

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
        <Flex vertical className="gap-3">
          {/* User Registration Settings — hidden until the backend
            * endpoint exists. The store (Users.loadUserRegistrationSettings
            * / updateUserRegistrationSettings) is a TODO stub that hard-
            * codes `true` and never calls the server; rendering the
            * toggle here was a UI lie (audit 03 B-4). Re-enable when
            * `GET/POST /api/users/registration-settings` ships
            * server-side. */}
          {/* <UserRegistrationSettings /> */}

          <Card
            title="Users"
            extra={
              <Can permission={Permissions.UsersCreate}>
                <Tooltip content="Create user">
                  <Button
                    variant="ghost"
                    icon={<Plus aria-hidden="true" />}
                    onClick={() =>
                      Stores.CreateUserDrawer.openCreateUserDrawer()
                    }
                    aria-label="Create user"
                  />
                </Tooltip>
              </Can>
            }
          >
            {loadingUsers ? (
              <Loading label="Loading users" />
            ) : users.length === 0 ? (
              <div>
                <Empty description="No users yet" />
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
                              <UserIcon />
                              <Text className="font-medium">
                                {user.username}
                              </Text>
                              <Badge tone={user.is_active ? 'success' : 'error'}>{user.is_active ? 'Active' : 'Inactive'}</Badge>
                            </Flex>
                          </div>
                          <div
                            className={'flex gap-1 items-center justify-end'}
                          >
                            {getUserActions(user)}
                          </div>
                        </div>

                        <Descriptions
                          size="sm"
                          column={3}
                          items={[
                            { key: 'email', label: 'Email', children: user.email },
                            { key: 'last_login', label: 'Last Login', children: user.last_login_at ? new Date(user.last_login_at).toLocaleDateString() : 'Never' },
                            { key: 'created', label: 'Created', children: new Date(user.created_at).toLocaleDateString() },
                          ]}
                        />
                      </div>
                    </div>
                    {index < users.length - 1 && <Separator className="my-0" />}
                  </div>
                ))}
              </div>
            )}

            {users.length > 0 && (
              <>
                <Separator className="mb-4" />
                <div className="flex justify-end">
                  <Pagination
              previousLabel="Previous page" nextLabel="Next page" pageLabel={(p) => `Page ${p}`} aria-label="Pagination"
                    current={storePage}
                    total={totalUsers}
                    pageSize={storePageSize}
                    showSizeChanger
              pageSizeLabel="Page size"
              onPageSizeChange={(size: number) => handlePageChange(1, size)}
                    showQuickJumper
              jumpLabel="Go to page"
                    showTotal={(total, range) =>
                      `${range[0]}-${range[1]} of ${total} users`
                    }
                    onChange={handlePageChange}
                    pageSizeOptions={[5, 10, 20, 50]}
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

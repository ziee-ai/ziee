import { Trash2, Pencil, Lock, Users, User as UserIcon } from 'lucide-react'
import {
  Badge,
  Button,
  Card,
  Switch,
  Descriptions,
  Empty,
  Flex,
  message,
  Separator,
  Text,
  Confirm,
  ErrorState,
} from '@ziee/kit'
import { ListPagination } from '@/components/common/ListPagination'
import { Loading } from '@/core/components/Loading'
import { useEffect, useState } from 'react'
import { Stores } from '@ziee/framework/stores'
import { AddButton } from '@/modules/settings/components/AddButton'
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
  // Which user's activate/deactivate confirmation is open (shared by the status
  // Switch + the Activate/Deactivate Button — both open the same Confirm).
  const [activeConfirmUserId, setActiveConfirmUserId] = useState<string | null>(
    null,
  )

  const canEdit = usePermission(Permissions.UsersEdit)
  const canResetPassword = usePermission(Permissions.UsersResetPassword)
  const canAssignGroups = usePermission(Permissions.GroupsAssignUsers)
  const canDelete = usePermission(Permissions.UsersDelete)
  const canToggleStatus = usePermission(Permissions.UsersToggleStatus)

  // Toast only user-action failures. A failed users LOAD renders as a
  // persistent ErrorState below (not toast-only), so only toast an error that
  // occurred against already-loaded data (a mutation). Both `usersError` and
  // `groupsError` can ALSO originate from the page's initial GET (the groups
  // list loads for the assign-group flow); in a full outage that load-failure
  // toast would stack on top of the users ErrorState. Gate BOTH on
  // `users.length > 0` so a cold load failure is shown ONLY by the in-place
  // ErrorState, and a drawer-mutation failure (page already populated) still
  // toasts.
  useEffect(() => {
    if (users.length === 0) return
    if (usersError) {
      message.error(usersError)
      Stores.Users.clearError()
    }
    if (groupsError) {
      message.error(groupsError)
      Stores.UserGroups.clearError()
    }
  }, [usersError, groupsError, users.length])

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
          <Switch
            data-standalone-control
            tooltip={user.is_active ? 'Deactivate user' : 'Activate user'}
            className={'mr-2!'}
            checked={user.is_active}
            onChange={() => setActiveConfirmUserId(user.id)}
            data-testid={`user-active-switch-${user.id}`}
          />
          <Confirm
            open={activeConfirmUserId === user.id}
            onOpenChange={open =>
              setActiveConfirmUserId(open ? user.id : null)
            }
            title={`${user.is_active ? 'Deactivate' : 'Activate'} this user?`}
            onConfirm={() => handleToggleActive(user.id)}
            okText="OK"
            cancelText="Cancel"
            data-testid={`user-toggle-active-confirm-${user.id}`}
          >
            <Button variant="ghost" size="default" data-testid={`user-toggle-active-button-${user.id}`}>
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
          data-testid={`user-edit-button-${user.id}`}
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
          data-testid={`user-reset-password-button-${user.id}`}
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
          data-testid={`user-groups-button-${user.id}`}
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
            data-testid={`user-delete-confirm-${user.id}`}
          >
            <Button
              variant="ghost"
              icon={<Trash2 aria-hidden="true" />}
              aria-label={`Delete ${user.username}`}
              data-testid={`user-delete-button-${user.id}`}
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
            data-testid="user-list-card"
            extra={
              <Can permission={Permissions.UsersCreate}>
                <AddButton
                  label="Create user"
                  onClick={() => Stores.CreateUserDrawer.openCreateUserDrawer()}
                  data-testid="user-create-open-button"
                />
              </Can>
            }
          >
            {loadingUsers ? (
              <Loading label="Loading users" />
            ) : users.length === 0 ? (
              usersError ? (
                <ErrorState
                  resource="users"
                  description="The user list couldn't be loaded. Check your connection and try again."
                  details={usersError}
                  onRetry={() => Stores.Users.loadUsers(storePage, storePageSize)}
                  data-testid="user-list-error"
                />
              ) : (
                <div>
                  <Empty description="No users yet" data-testid="user-list-empty" />
                </div>
              )
            ) : (
              <div>
                {users.map((user, index) => (
                  <div key={user.id} data-testid={`user-row-${user.username}`}>
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
                              <Badge tone={user.is_active ? 'success' : 'error'} data-testid={`user-status-badge-${user.id}`}>{user.is_active ? 'Active' : 'Inactive'}</Badge>
                            </Flex>
                          </div>
                          {/* Left-align the wrapped action list on mobile
                              so a lone wrapped action (e.g. "Groups") isn't
                              orphaned right; right-align inline from sm up. */}
                          <div
                            className={'flex flex-wrap gap-1 items-center justify-start sm:justify-end'}
                          >
                            {getUserActions(user)}
                          </div>
                        </div>

                        <Descriptions
                          size="sm"
                          column={3}
                          data-testid={`user-descriptions-${user.id}`}
                          items={[
                            { key: 'email', label: 'Email', children: user.email },
                            { key: 'last_login', label: 'Last Login', children: user.last_login_at ? new Date(user.last_login_at).toLocaleDateString() : 'Never' },
                            { key: 'created', label: 'Created', children: new Date(user.created_at).toLocaleDateString() },
                          ]}
                        />
                      </div>
                    </div>
                    {index < users.length - 1 && <Separator className="my-4" />}
                  </div>
                ))}
              </div>
            )}

            {users.length > 0 && (
              <>
                <ListPagination
          data-testid="user-list-pagination"
          current={storePage}
          total={totalUsers}
          pageSize={storePageSize}
          onChange={handlePageChange}
          onPageSizeChange={(size: number) => handlePageChange(1, size)}
          itemNoun="users"
          aria-label="Pagination"
        />
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

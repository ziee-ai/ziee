import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  User,
  CreateUserRequest,
  UpdateUserRequest,
  Group,
  CreateGroupRequest,
  UpdateGroupRequest,
} from '@/api-client/types'

// =====================================================
// Users Store
// =====================================================

interface UsersState {
  // Data
  users: User[]
  total: number
  currentPage: number
  pageSize: number
  isInitialized: boolean

  // User registration settings
  userRegistrationEnabled: boolean
  registrationSettingsInitialized: boolean
  loadingRegistrationSettings: boolean

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // Error state
  error: string | null
  __init__: {
    users: () => Promise<void>
  }
}

export const useUsersStore = create<UsersState>()(
  subscribeWithSelector(
    (): UsersState => ({
      // Initial state
      users: [],
      total: 0,
      currentPage: 1,
      pageSize: 10,
      isInitialized: false,
      userRegistrationEnabled: true,
      registrationSettingsInitialized: false,
      loadingRegistrationSettings: false,
      loading: false,
      creating: false,
      updating: false,
      deleting: false,
      error: null,
      __init__: {
        users: async () => {
          loadUsers()
        },
      },
    }),
  ),
)

// User actions
export const loadUsers = async (
  page?: number,
  pageSize?: number,
): Promise<void> => {
  try {
    const currentState = useUsersStore.getState()
    const requestPage = page || currentState.currentPage
    const requestPageSize = pageSize || currentState.pageSize

    // Skip if already initialized and loading first page without explicit page parameter
    if (currentState.isInitialized && currentState.loading && !page) {
      return
    }

    useUsersStore.setState({ loading: true, error: null })

    const response = await ApiClient.User.list({
      page: requestPage,
      per_page: requestPageSize,
    })

    useUsersStore.setState({
      users: response.users,
      total: response.total,
      currentPage: response.page,
      pageSize: response.per_page,
      isInitialized: true,
      loading: false,
    })
  } catch (error) {
    useUsersStore.setState({
      error: error instanceof Error ? error.message : 'Failed to load users',
      loading: false,
    })
    throw error
  }
}

export const createUser = async (
  data: CreateUserRequest,
): Promise<User | undefined> => {
  const state = useUsersStore.getState()
  if (state.creating) {
    return
  }

  try {
    useUsersStore.setState({ creating: true, error: null })

    const user = await ApiClient.User.create(data)

    useUsersStore.setState(state => ({
      users: [...state.users, user],
      total: state.total + 1,
      creating: false,
    }))

    return user
  } catch (error) {
    useUsersStore.setState({
      error: error instanceof Error ? error.message : 'Failed to create user',
      creating: false,
    })
    throw error
  }
}

export const updateUser = async (
  id: string,
  data: UpdateUserRequest,
): Promise<User | undefined> => {
  const state = useUsersStore.getState()
  if (state.updating) {
    return
  }

  try {
    useUsersStore.setState({ updating: true, error: null })

    const user = await ApiClient.User.update({
      user_id: id,
      ...data,
    })

    useUsersStore.setState(state => ({
      users: state.users.map(u => (u.id === id ? user : u)),
      updating: false,
    }))

    return user
  } catch (error) {
    useUsersStore.setState({
      error: error instanceof Error ? error.message : 'Failed to update user',
      updating: false,
    })
    throw error
  }
}

export const resetUserPassword = async (
  id: string,
  newPassword: string,
): Promise<void> => {
  const state = useUsersStore.getState()
  if (state.updating) {
    return
  }

  try {
    useUsersStore.setState({ updating: true, error: null })

    await ApiClient.User.resetPassword({
      user_id: id,
      new_password: newPassword,
    })

    useUsersStore.setState({ updating: false })
  } catch (error) {
    useUsersStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to reset password',
      updating: false,
    })
    throw error
  }
}

export const toggleUserActiveStatus = async (id: string): Promise<void> => {
  const state = useUsersStore.getState()
  if (state.updating) {
    return
  }

  try {
    useUsersStore.setState({ updating: true, error: null })

    await ApiClient.User.toggleActive({
      user_id: id,
    })

    useUsersStore.setState(state => ({
      users: state.users.map(u =>
        u.id === id ? { ...u, is_active: !u.is_active } : u,
      ),
      updating: false,
    }))
  } catch (error) {
    useUsersStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to toggle user status',
      updating: false,
    })
    throw error
  }
}

export const deleteUser = async (id: string): Promise<void> => {
  const state = useUsersStore.getState()
  if (state.deleting) {
    return
  }

  try {
    useUsersStore.setState({ deleting: true, error: null })

    await ApiClient.User.delete({
      user_id: id,
    })

    useUsersStore.setState(state => ({
      users: state.users.filter(u => u.id !== id),
      total: state.total - 1,
      deleting: false,
    }))
  } catch (error) {
    useUsersStore.setState({
      error: error instanceof Error ? error.message : 'Failed to delete user',
      deleting: false,
    })
    throw error
  }
}

export const clearUsersStoreError = (): void => {
  useUsersStore.setState({ error: null })
}

// Registration settings (placeholder - requires backend endpoints)
export const loadUserRegistrationSettings = async (): Promise<void> => {
  const state = useUsersStore.getState()
  if (
    state.registrationSettingsInitialized ||
    state.loadingRegistrationSettings
  ) {
    return
  }
  try {
    useUsersStore.setState({
      loadingRegistrationSettings: true,
      error: null,
    })

    // TODO: Replace with actual API call when backend endpoint exists
    // const { enabled } = await callAsync('GET /api/users/registration-settings', {})

    useUsersStore.setState({
      userRegistrationEnabled: true, // Default for now
      registrationSettingsInitialized: true,
      loadingRegistrationSettings: false,
    })
  } catch (error) {
    useUsersStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to load registration settings',
      loadingRegistrationSettings: false,
    })
    throw error
  }
}

export const updateUserRegistrationSettings = async (
  enabled: boolean,
): Promise<void> => {
  const state = useUsersStore.getState()
  if (state.updating) {
    return
  }

  try {
    useUsersStore.setState({ updating: true, error: null })

    // TODO: Replace with actual API call when backend endpoint exists
    // await callAsync('POST /api/users/registration-settings', { enabled })

    useUsersStore.setState({
      userRegistrationEnabled: enabled,
      updating: false,
    })
  } catch (error) {
    useUsersStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to update registration settings',
      updating: false,
    })
    throw error
  }
}

// =====================================================
// User Groups Store
// =====================================================

interface GroupMember {
  id: string
  username: string
  email: string
  is_active: boolean
  joined_at: string
}

interface UserGroupsState {
  // Data
  groups: Group[]
  currentGroupMembers: GroupMember[]
  total: number
  currentPage: number
  pageSize: number
  isInitialized: boolean
  currentGroupId: string | null

  // Loading states
  loading: boolean
  loadingGroups: boolean
  loadingGroupMembers: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // Error state
  error: string | null

  __init__: {
    groups: () => Promise<void>
  }
}

export const useUserGroupsStore = create<UserGroupsState>()(
  subscribeWithSelector(
    (): UserGroupsState => ({
      // Initial state
      groups: [],
      currentGroupMembers: [],
      total: 0,
      currentPage: 1,
      pageSize: 10,
      isInitialized: false,
      currentGroupId: null,
      loading: false,
      loadingGroups: false,
      loadingGroupMembers: false,
      creating: false,
      updating: false,
      deleting: false,
      error: null,
      __init__: {
        groups: async () => loadUserGroups(),
      },
    }),
  ),
)

// User Group actions (placeholder - requires backend endpoints)
export const loadUserGroups = async (
  page?: number,
  pageSize?: number,
): Promise<void> => {
  try {
    const currentState = useUserGroupsStore.getState()
    const requestPage = page || currentState.currentPage
    const requestPageSize = pageSize || currentState.pageSize

    // Skip if already initialized and loading first page without explicit page parameter
    if (currentState.isInitialized && currentState.loadingGroups && !page) {
      return
    }

    useUserGroupsStore.setState({ loadingGroups: true, error: null })

    const response = await ApiClient.UserGroup.list({
      page: requestPage,
      per_page: requestPageSize,
    })

    useUserGroupsStore.setState({
      groups: response.groups,
      total: response.total,
      currentPage: response.page,
      pageSize: response.per_page,
      isInitialized: true,
      loadingGroups: false,
    })
  } catch (error) {
    useUserGroupsStore.setState({
      error: error instanceof Error ? error.message : 'Failed to load groups',
      loadingGroups: false,
    })
    throw error
  }
}

export const createUserGroup = async (
  data: CreateGroupRequest,
): Promise<Group | undefined> => {
  const state = useUserGroupsStore.getState()
  if (state.creating) {
    return
  }

  try {
    useUserGroupsStore.setState({ creating: true, error: null })

    const group = await ApiClient.UserGroup.create(data)

    useUserGroupsStore.setState(state => ({
      groups: [...state.groups, group],
      total: state.total + 1,
      creating: false,
    }))

    return group
  } catch (error) {
    useUserGroupsStore.setState({
      error: error instanceof Error ? error.message : 'Failed to create group',
      creating: false,
    })
    throw error
  }
}

export const updateUserGroup = async (
  id: string,
  data: UpdateGroupRequest,
): Promise<Group | undefined> => {
  const state = useUserGroupsStore.getState()
  if (state.updating) {
    return
  }

  try {
    useUserGroupsStore.setState({ updating: true, error: null })

    const group = await ApiClient.UserGroup.update({
      group_id: id,
      ...data,
    })

    useUserGroupsStore.setState(state => ({
      groups: state.groups.map(g => (g.id === id ? group : g)),
      updating: false,
    }))

    return group
  } catch (error) {
    useUserGroupsStore.setState({
      error: error instanceof Error ? error.message : 'Failed to update group',
      updating: false,
    })
    throw error
  }
}

export const deleteUserGroup = async (id: string): Promise<void> => {
  const state = useUserGroupsStore.getState()
  if (state.deleting) {
    return
  }

  try {
    useUserGroupsStore.setState({ deleting: true, error: null })

    await ApiClient.UserGroup.delete({
      group_id: id,
    })

    useUserGroupsStore.setState(state => ({
      groups: state.groups.filter(g => g.id !== id),
      total: state.total - 1,
      deleting: false,
    }))
  } catch (error) {
    useUserGroupsStore.setState({
      error: error instanceof Error ? error.message : 'Failed to delete group',
      deleting: false,
    })
    throw error
  }
}

export const loadUserGroupMembers = async (groupId: string): Promise<void> => {
  try {
    const currentState = useUserGroupsStore.getState()

    // Skip if already loading members for the same group
    if (
      currentState.loadingGroupMembers &&
      currentState.currentGroupId === groupId
    ) {
      return
    }

    useUserGroupsStore.setState({
      loadingGroupMembers: true,
      error: null,
      currentGroupId: groupId,
    })

    const response = await ApiClient.UserGroup.getMembers({
      group_id: groupId,
      page: 1,
      per_page: 50,
    })

    useUserGroupsStore.setState({
      currentGroupMembers: response.users.map(u => ({
        id: u.id,
        username: u.username,
        email: u.email,
        is_active: u.is_active,
        joined_at: new Date().toISOString(),
      })),
      loadingGroupMembers: false,
    })
  } catch (error) {
    useUserGroupsStore.setState({
      error:
        error instanceof Error ? error.message : 'Failed to load group members',
      loadingGroupMembers: false,
    })
    throw error
  }
}

export const assignUserToUserGroup = async (
  userId: string,
  groupId: string,
): Promise<void> => {
  const state = useUserGroupsStore.getState()
  if (state.updating) {
    return
  }

  try {
    useUserGroupsStore.setState({ updating: true, error: null })

    await ApiClient.UserGroup.assignUser({
      user_id: userId,
      group_id: groupId,
    })

    // Reload group members if we're viewing this group
    if (state.currentGroupId === groupId) {
      await loadUserGroupMembers(groupId)
    }

    useUserGroupsStore.setState({ updating: false })
  } catch (error) {
    useUserGroupsStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to assign user to group',
      updating: false,
    })
    throw error
  }
}

export const removeUserFromUserGroup = async (
  userId: string,
  groupId: string,
): Promise<void> => {
  const state = useUserGroupsStore.getState()
  if (state.updating) {
    return
  }

  try {
    useUserGroupsStore.setState({ updating: true, error: null })

    await ApiClient.UserGroup.removeUser({
      user_id: userId,
      group_id: groupId,
    })

    // Remove from current group members list
    useUserGroupsStore.setState(state => ({
      currentGroupMembers: state.currentGroupMembers.filter(
        m => m.id !== userId,
      ),
      updating: false,
    }))
  } catch (error) {
    useUserGroupsStore.setState({
      error:
        error instanceof Error
          ? error.message
          : 'Failed to remove user from group',
      updating: false,
    })
    throw error
  }
}

export const clearUserGroupsStoreError = (): void => {
  useUserGroupsStore.setState({ error: null })
}

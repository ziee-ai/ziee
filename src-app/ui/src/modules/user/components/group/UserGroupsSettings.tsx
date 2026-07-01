import { Plus } from 'lucide-react'
import {
  Button,
  Empty,
  Flex,
  Form,
  FormField,
  useForm,
  zodResolver,
  Input,
  Textarea,
  Pagination,
  Tooltip,
  message,
} from '@/components/ui'
import { z } from 'zod'
import { Loading } from '@/core/components/Loading'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useEffect, useState } from 'react'
import { Stores } from '@/core/stores'
import { Can, usePermission } from '@/core/permissions'
import { Permissions, type CreateGroupRequest, type Group } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer.tsx'
import { EditUserGroupDrawer } from '@/modules/user/components/group/EditUserGroupDrawer.tsx'
import { GroupMembersDrawer } from '@/modules/user/components/group/GroupMembersDrawer.tsx'
import { GroupListItem } from '@/modules/user/components/group/GroupListItem.tsx'
import { PermissionsField } from '@/modules/user/components/PermissionsField.tsx'

interface CreateGroupFormValues {
  name: string
  description?: string
  permissions?: string[]
}

const schema = z.object({
  name: z.string().min(1, 'Please enter group name'),
  description: z.string().optional(),
  permissions: z.array(z.string()).optional(),
})

export function UserGroupsSettings() {
  const {
    groups,
    total: totalGroups,
    currentPage: storePage,
    pageSize: storePageSize,
    loadingGroups,
    error,
  } = Stores.UserGroups

  const [createModalVisible, setCreateModalVisible] = useState(false)
  const createForm = useForm<CreateGroupFormValues>({
    resolver: zodResolver(schema),
    defaultValues: { name: '', description: '', permissions: [] },
  })
  const canCreate = usePermission(Permissions.GroupsCreate)

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.UserGroups.clearError()
    }
  }, [error])

  const handleCreateGroup = async (values: CreateGroupFormValues) => {
    try {
      const groupData: CreateGroupRequest = {
        name: values.name,
        description: values.description,
        permissions: values.permissions ?? [],
      }
      await Stores.UserGroups.createUserGroup(groupData)
      message.success('User group created successfully')
      setCreateModalVisible(false)
      createForm.reset()
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
            variant="default"
            size="icon"
            icon={<Plus aria-hidden="true" />}
            onClick={() => setCreateModalVisible(true)}
            aria-label="Create group"
            data-testid="user-groups-create-button"
          />
        </Tooltip>
      </Can>
    </div>
  )

  return (
    <SettingsPageContainer title={titleWithButton}>
      {loadingGroups ? (
        <Loading />
      ) : groups.length === 0 ? (
        <Empty description="No user groups yet" data-testid="user-groups-empty" />
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
              data-testid="user-groups-pagination"
              aria-label="Groups pagination"
              previousLabel="Previous page"
              nextLabel="Next page"
              pageLabel={(page) => `Page ${page}`}
              current={storePage}
              total={totalGroups}
              pageSize={storePageSize}
              showSizeChanger
              pageSizeLabel="Page size"
              onPageSizeChange={(size) => handlePageChange(1, size)}
              showQuickJumper
              jumpLabel="Go to page"
              showTotal={(total, range) =>
                `${range[0]}-${range[1]} of ${total} groups`
              }
              onChange={(page) => handlePageChange(page)}
              pageSizeOptions={[5, 10, 20, 50]}
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
          createForm.reset()
        }}
        footer={null}
        size={600}
        mask={{ closable: false }}
      >
        <Form
          form={createForm}
          layout="vertical"
          onSubmit={handleCreateGroup}
          disabled={!canCreate}
          data-testid="user-create-group-form"
        >
          <FormField name="name" label="Group Name" required>
            <Input placeholder="Enter group name" data-testid="user-create-group-name-input" />
          </FormField>
          <FormField name="description" label="Description">
            <Textarea rows={3} placeholder="Enter group description" data-testid="user-create-group-description-textarea" />
          </FormField>
          <FormField name="permissions" label="Permissions">
            <PermissionsField disabled={!canCreate} />
          </FormField>

          <Flex className="justify-end gap-2">
            <Button
              variant="outline"
              onClick={() => {
                setCreateModalVisible(false)
                createForm.reset()
              }}
              data-testid="user-create-group-cancel-button"
            >
              {canCreate ? 'Cancel' : 'Close'}
            </Button>
            {canCreate && (
              <Button type="submit" data-testid="user-create-group-submit-button">
                Create
              </Button>
            )}
          </Flex>
        </Form>
      </Drawer>

      {/* Edit Group Drawer */}
      <EditUserGroupDrawer />

      {/* Group Members Drawer */}
      <GroupMembersDrawer />
    </SettingsPageContainer>
  )
}

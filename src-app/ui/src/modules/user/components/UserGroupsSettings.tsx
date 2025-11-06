import {
  DeleteOutlined,
  EditOutlined,
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
  Spin,
  Tag,
  Typography,
} from 'antd'
import { Drawer } from '@/components/common/Drawer.tsx'
import { useEffect, useState } from 'react'
import { Stores } from '@/core/stores'
import type { CreateGroupRequest, Group } from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer.tsx'
import { EditUserGroupDrawer } from './EditUserGroupDrawer.tsx'

const { Text } = Typography
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
    currentGroupMembers,
    total: totalGroups,
    currentPage: storePage,
    pageSize: storePageSize,
    loadingGroups,
    loadingGroupMembers,
    error,
  } = Stores.UserGroups

  const [createModalVisible, setCreateModalVisible] = useState(false)
  const [editModalVisible, setEditModalVisible] = useState(false)
  const [membersDrawerVisible, setMembersDrawerVisible] = useState(false)
  const [selectedGroup, setSelectedGroup] = useState<Group | null>(null)
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

  const handleEditSuccess = () => {
    setEditModalVisible(false)
    setSelectedGroup(null)
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

  const handleViewMembers = async (group: Group) => {
    setSelectedGroup(group)
    setMembersDrawerVisible(true)

    try {
      await Stores.UserGroups.loadUserGroupMembers(group.id)
    } catch (error) {
      console.error('Failed to fetch group members:', error)
      // Error is handled by the store
    }
  }

  const openEditModal = (group: Group) => {
    setSelectedGroup(group)
    setEditModalVisible(true)
  }

  const getGroupActions = (group: Group) => {
    const actions: React.ReactNode[] = []

    actions.push(
      <Button
        key="members"
        type="text"
        icon={<UserOutlined />}
        onClick={() => handleViewMembers(group)}
      >
        Members
      </Button>,
    )

    actions.push(
      <Button
        key="edit"
        type="text"
        icon={<EditOutlined />}
        onClick={() => openEditModal(group)}
      >
        Edit
      </Button>,
    )

    actions.push(
      <Popconfirm
        key="delete"
        title="Are you sure you want to delete this group?"
        onConfirm={() => handleDeleteGroup(group.id)}
        okText="Yes"
        cancelText="No"
      >
        <Button type="text" danger icon={<DeleteOutlined />}>
          Delete
        </Button>
      </Popconfirm>,
    )

    return actions.filter(Boolean)
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
            <div>
              {groups.map((group, index) => (
                <div key={group.id}>
                  <div className="flex items-start gap-3 flex-wrap">
                    {/* Group Info */}
                    <div className="flex-1">
                      <div className="flex items-center gap-2 mb-2 flex-wrap">
                        <div className={'flex-1 min-w-48'}>
                          <Flex className="gap-2 items-center">
                            <TeamOutlined />
                            <Text className="font-medium">{group.name}</Text>
                            {group.is_system && (
                              <Tag color="orange">System</Tag>
                            )}
                            <Badge
                              status={group.is_active ? 'success' : 'error'}
                              text={group.is_active ? 'Active' : 'Inactive'}
                            />
                          </Flex>
                        </div>
                        <div className={'flex gap-1 items-center justify-end'}>
                          {getGroupActions(group)}
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
                        <Descriptions.Item label="Description">
                          {group.description || 'No description'}
                        </Descriptions.Item>
                        <Descriptions.Item label="Permissions">
                          <Text code>
                            {Object.keys(group.permissions || {}).length}{' '}
                            permissions
                          </Text>
                        </Descriptions.Item>
                        <Descriptions.Item label="Created">
                          {new Date(group.created_at).toLocaleDateString()}
                        </Descriptions.Item>
                      </Descriptions>
                    </div>
                  </div>
                  {index < groups.length - 1 && <Divider className="my-0" />}
                </div>
              ))}
            </div>
          )}

          {groups.length > 0 && (
            <>
              <Divider className="mb-4" />
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
        <EditUserGroupDrawer
          group={selectedGroup}
          open={editModalVisible}
          onClose={() => {
            setEditModalVisible(false)
            setSelectedGroup(null)
          }}
          onSuccess={handleEditSuccess}
        />

        {/* Group Members Drawer */}
        <Drawer
          title={`Members of ${selectedGroup?.name}`}
          placement="right"
          onClose={() => setMembersDrawerVisible(false)}
          open={membersDrawerVisible}
          width={400}
        >
          <List
            loading={loadingGroupMembers}
            dataSource={currentGroupMembers}
            renderItem={user => (
              <List.Item>
                <List.Item.Meta
                  avatar={<UserOutlined />}
                  title={user.username}
                  description={
                    <div>
                      <div>{user.email}</div>
                      <Tag color={user.is_active ? 'green' : 'red'}>
                        {user.is_active ? 'Active' : 'Inactive'}
                      </Tag>
                    </div>
                  }
                />
              </List.Item>
            )}
          />
        </Drawer>
      </div>
    </SettingsPageContainer>
  )
}

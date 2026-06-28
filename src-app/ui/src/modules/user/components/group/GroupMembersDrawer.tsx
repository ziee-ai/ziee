import { User } from 'lucide-react'
import { List, Tag } from 'antd'
import { useEffect } from 'react'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'

export function GroupMembersDrawer() {
  const { isOpen: open, selectedGroup: group } = Stores.GroupMembersDrawer
  const { currentGroupMembers, loadingGroupMembers } = Stores.UserGroups

  // Load group members when group changes
  useEffect(() => {
    if (group && open) {
      Stores.UserGroups.loadUserGroupMembers(group.id)
    }
  }, [group, open])

  const handleClose = () => {
    Stores.GroupMembersDrawer.closeGroupMembersDrawer()
  }

  return (
    <Drawer
      title={group ? `Members of ${group.name}` : 'Group Members'}
      placement="right"
      onClose={handleClose}
      open={open}
      size={600}
    >
      <List
        loading={loadingGroupMembers}
        dataSource={currentGroupMembers}
        renderItem={user => (
          <List.Item>
            <List.Item.Meta
              avatar={<User aria-hidden="true" />}
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
  )
}

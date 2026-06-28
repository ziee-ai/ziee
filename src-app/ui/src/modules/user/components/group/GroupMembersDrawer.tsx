import { User } from 'lucide-react'
import { useEffect } from 'react'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { List, Tag } from '@/components/ui'

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
        rowKey="id"
        renderItem={user => (
          <div className="flex items-start gap-3">
            <User aria-hidden="true" className="mt-0.5 shrink-0" />
            <div className="flex flex-col gap-1">
              <span className="font-medium">{user.username}</span>
              <div>
                <div>{user.email}</div>
                <Tag tone={user.is_active ? 'success' : 'error'}>
                  {user.is_active ? 'Active' : 'Inactive'}
                </Tag>
              </div>
            </div>
          </div>
        )}
      />
    </Drawer>
  )
}

import { Trash2, Pencil, Users, User } from 'lucide-react'
import {
  Badge,
  Button,
  Card,
  Confirm,
  Descriptions,
  Flex,
  Tag,
  Text,
  type DescriptionsItem,
} from '@/components/ui'
import { Permissions, type Group } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { WidgetRenderer } from '@/core/components/LazyComponentRenderer'
import type { GroupWidget } from '@/modules/user/types/GroupWidget'

interface GroupListItemProps {
  group: Group
  showDivider?: boolean
  onEdit: (group: Group) => void
  onDelete: (groupId: string) => void
  onViewMembers: (group: Group) => void
}

export function GroupListItem({
  group,
  onEdit,
  onDelete,
  onViewMembers,
}: GroupListItemProps) {
  // Get items for the userGroup slot from global registry
  const { slots } = Stores.ModuleSystem
  const userGroupWidgets = (slots.get('userGroup') || []) as GroupWidget[]

  const canReadMembers = usePermission(Permissions.GroupsRead)
  const canEdit = usePermission(Permissions.GroupsEdit)
  const canDelete = usePermission(Permissions.GroupsDelete)

  // Sort items by order
  const registeredWidgets = [...userGroupWidgets].sort(
    (a, b) => a.order - b.order,
  )

  const getGroupActions = () => {
    const actions: React.ReactNode[] = []

    if (canReadMembers) {
      actions.push(
        <Button
          key="members"
          variant="ghost"
          icon={<User aria-hidden="true" />}
          onClick={() => onViewMembers(group)}
          aria-label={`View members of ${group.name}`}
        >
          Members
        </Button>,
      )
    }

    if (canEdit) {
      actions.push(
        <Button
          key="edit"
          variant="ghost"
          icon={<Pencil aria-hidden="true" />}
          onClick={() => onEdit(group)}
          aria-label={`Edit ${group.name}`}
        >
          Edit
        </Button>,
      )
    }

    // Hide Delete on system groups regardless of permission — backend
    // refuses to delete system groups and the UI shouldn't pretend.
    if (canDelete && !group.is_system) {
      actions.push(
        <Confirm
          key="delete"
          title="Are you sure you want to delete this group?"
          onConfirm={() => onDelete(group.id)}
          okText="Delete"
          cancelText="Cancel"
        >
          <Button
            variant="destructive"
            icon={<Trash2 aria-hidden="true" />}
            aria-label={`Delete ${group.name}`}
          >
            Delete
          </Button>
        </Confirm>,
      )
    }

    return actions.filter(Boolean)
  }

  const descriptionItems: DescriptionsItem[] = [
    {
      key: 'description',
      label: 'Description',
      children: group.description || 'No description',
      span: 2,
    },
    {
      key: 'permissions',
      label: 'Permissions',
      children: (
        <Text className="font-mono text-xs">
          {Object.keys(group.permissions || {}).length} permissions
        </Text>
      ),
    },
    {
      key: 'created',
      label: 'Created',
      children: new Date(group.created_at).toLocaleDateString(),
    },
  ]

  return (
    <Card>
      <div className="flex items-start gap-3 flex-wrap">
        {/* Group Info */}
        <div className="flex-1">
          <div className="flex items-center gap-2 mb-2 flex-wrap">
            <div className={'flex-1 min-w-48'}>
              <Flex className="gap-2 items-center">
                <Users aria-hidden="true" />
                <Text className="font-medium">{group.name}</Text>
                {group.is_system && <Tag tone="warning">System</Tag>}
                <Badge color={group.is_active ? 'green' : 'red'} />
                <Text>{group.is_active ? 'Active' : 'Inactive'}</Text>
              </Flex>
            </div>
            <div className={'flex gap-1 items-center justify-end'}>
              {getGroupActions()}
            </div>
          </div>

          {/*
            Layout:
              row 1: Description (full width, span = column count)
              row 2: Permissions  |  Created  (half each)
            On xs the grid collapses to one column so all three
            stack naturally.
          */}
          <Descriptions
            size="sm"
            column={2}
            className="[&_.ant-descriptions-item-label]:text-[12px] [&_.ant-descriptions-item-content]:text-[12px]"
            items={descriptionItems}
          />
        </div>
      </div>

      {/* Render registered widgets */}
      {registeredWidgets.length > 0 && (
        <div className="mt-3 flex gap-2 flex-col">
          {registeredWidgets.map((widget, index) => (
            <WidgetRenderer key={index} widget={widget} props={{ group }} />
          ))}
        </div>
      )}
    </Card>
  )
}

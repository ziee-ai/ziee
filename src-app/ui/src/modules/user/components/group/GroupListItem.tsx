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
          data-testid={`user-group-members-button-${group.id}`}
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
          data-testid={`user-group-edit-button-${group.id}`}
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
          data-testid={`user-group-delete-confirm-${group.id}`}
        >
          <Button
            variant="ghost"
            icon={<Trash2 aria-hidden="true" />}
            aria-label={`Delete ${group.name}`}
            data-testid={`user-group-delete-button-${group.id}`}
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
    <Card data-testid={`user-group-card-${group.id}`}>
      <div className="flex items-start gap-3 flex-wrap" data-testid={`user-group-row-${group.name}`}>
        {/* Group Info */}
        <div className="flex-1">
          <div className="flex items-center gap-2 mb-2 flex-wrap">
            <div className={'flex-1 min-w-48'}>
              {/* flex-wrap so the status pill reflows onto its own line on
                  narrow widths instead of clipping "Active" → "Ac". The badge +
                  label stay together as one nowrap unit across the wrap. */}
              <Flex className="gap-2 items-center flex-wrap">
                <Users aria-hidden="true" />
                <Text className="font-medium">{group.name}</Text>
                {group.is_system && <Tag variant="outline" tone="warning" data-testid={`user-group-system-tag-${group.id}`}>System</Tag>}
                <span className="inline-flex items-center gap-2 whitespace-nowrap">
                  <Badge color={group.is_active ? 'green' : 'red'} data-testid={`user-group-active-badge-${group.id}`} />
                  <Text data-testid={`user-group-status-text-${group.id}`}>{group.is_active ? 'Active' : 'Inactive'}</Text>
                </span>
              </Flex>
            </div>
            <div className={'flex flex-wrap gap-1 items-center justify-end'}>
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
            className="text-[12px]"
            items={descriptionItems}
            data-testid={`user-group-descriptions-${group.id}`}
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

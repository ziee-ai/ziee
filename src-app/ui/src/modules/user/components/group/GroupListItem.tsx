import {
  DeleteOutlined,
  EditOutlined,
  TeamOutlined,
  UserOutlined,
} from '@ant-design/icons'
import {
  Badge,
  Button,
  Card,
  Descriptions,
  Flex,
  Popconfirm,
  Tag,
  Typography,
} from 'antd'
import type { Group } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { WidgetRenderer } from '@/core/components/LazyComponentRenderer'
import type { GroupWidget } from '../../types/GroupWidget'

const { Text } = Typography

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

  // Sort items by order
  const registeredWidgets = [...userGroupWidgets].sort((a, b) => a.order - b.order)

  const getGroupActions = () => {
    const actions: React.ReactNode[] = []

    actions.push(
      <Button
        key="members"
        type="text"
        icon={<UserOutlined aria-hidden="true" />}
        onClick={() => onViewMembers(group)}
        aria-label={`View members of ${group.name}`}
      >
        Members
      </Button>,
    )

    actions.push(
      <Button
        key="edit"
        type="text"
        icon={<EditOutlined aria-hidden="true" />}
        onClick={() => onEdit(group)}
        aria-label={`Edit ${group.name}`}
      >
        Edit
      </Button>,
    )

    actions.push(
      <Popconfirm
        key="delete"
        title="Are you sure you want to delete this group?"
        onConfirm={() => onDelete(group.id)}
        okText="Yes"
        cancelText="No"
      >
        <Button
          type="text"
          danger
          icon={<DeleteOutlined aria-hidden="true" />}
          aria-label={`Delete ${group.name}`}
        >
          Delete
        </Button>
      </Popconfirm>,
    )

    return actions.filter(Boolean)
  }

  return (
    <Card>
      <div className="flex items-start gap-3 flex-wrap">
        {/* Group Info */}
        <div className="flex-1">
            <div className="flex items-center gap-2 mb-2 flex-wrap">
              <div className={'flex-1 min-w-48'}>
                <Flex className="gap-2 items-center">
                  <TeamOutlined aria-hidden="true" />
                  <Text className="font-medium">{group.name}</Text>
                  {group.is_system && <Tag color="orange">System</Tag>}
                  <Badge
                    status={group.is_active ? 'success' : 'error'}
                    text={group.is_active ? 'Active' : 'Inactive'}
                  />
                </Flex>
              </div>
              <div className={'flex gap-1 items-center justify-end'}>
                {getGroupActions()}
              </div>
            </div>

            <Descriptions
              size="small"
              column={{ xs: 1, sm: 2, md: 3 }}
              colon={false}
              styles={{
                label: { fontSize: '12px' },
                content: { fontSize: '12px' },
              }}
            >
              <Descriptions.Item label="Description">
                {group.description || 'No description'}
              </Descriptions.Item>
              <Descriptions.Item label="Permissions">
                <Text code>
                  {Object.keys(group.permissions || {}).length} permissions
                </Text>
              </Descriptions.Item>
              <Descriptions.Item label="Created">
                {new Date(group.created_at).toLocaleDateString()}
              </Descriptions.Item>
            </Descriptions>
          </div>
        </div>

        {/* Render registered widgets */}
        {registeredWidgets.length > 0 && (
          <div className="mt-3 flex gap-2 flex-col">
            {registeredWidgets.map((widget, index) => (
              <WidgetRenderer
                key={index}
                widget={widget}
                props={{ group }}
              />
            ))}
          </div>
        )}

    </Card>
  )
}

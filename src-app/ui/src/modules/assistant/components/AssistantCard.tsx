import {
  CalendarOutlined,
  DeleteOutlined,
  EditOutlined,
} from '@ant-design/icons'
import { Button, Card, Flex, Popconfirm, Tag, Typography } from 'antd'
import { Permissions, type Assistant } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'

dayjs.extend(relativeTime)

const { Text } = Typography

interface AssistantCardProps {
  assistant: Assistant
  onEdit: (assistant: Assistant) => void
  onDelete: () => void
}

export function AssistantCard({
  assistant,
  onEdit,
  onDelete,
}: AssistantCardProps) {
  // User assistants only (template list has its own page with its own
  // gating). Edit also opens the drawer in view-only when missing the
  // perm — Form's `disabled` flag downstream handles read-only display.
  const canEdit = usePermission(Permissions.AssistantsEdit)
  const canDelete = usePermission(Permissions.AssistantsDelete)

  const handleEdit = () => {
    onEdit(assistant)
  }

  const handleCardClick = () => {
    onEdit(assistant)
  }

  return (
    <Card
      className="cursor-pointer relative group hover:!shadow-md transition-shadow h-full"
      classNames={{
        body: '!px-3 !pb-0 !py-2 flex gap-2 flex-col',
      }}
      hoverable
      onClick={handleCardClick}
      data-test-assistant-name={assistant.name}
    >
      <Flex className="h-full flex-col flex-1">
        {/* Header with name and tags */}
        <Typography.Text strong className="m-0 pr-2">
          {assistant.name}
        </Typography.Text>

        {/* Tags */}
        {(assistant.is_default || !assistant.enabled) && (
          <div className="mb-2">
            <Flex className="gap-1">
              {assistant.is_default && (
                <Tag color="blue" className="text-xs">
                  Default
                </Tag>
              )}
              {!assistant.enabled && (
                <Tag color="red" className="text-xs">
                  Inactive
                </Tag>
              )}
            </Flex>
          </div>
        )}

        {/* Description */}
        {assistant.description && (
          <div className="mb-3">
            <Text type="secondary" className="text-sm line-clamp-2">
              {assistant.description}
            </Text>
          </div>
        )}

        {/* Stats and date - pushed to bottom */}
        <div
          style={{
            marginTop: assistant.description ? 'auto' : '12px',
          }}
        >
          {/* Last updated */}
          <div className="mb-2">
            <Flex align="center" gap="small">
              <CalendarOutlined className="text-gray-400" />
              <Text type="secondary" className="text-xs">
                Updated {dayjs(assistant.updated_at).fromNow()}
              </Text>
            </Flex>
          </div>
        </div>

        {(canEdit || canDelete) && (
          <div className="absolute top-2 right-2 flex gap-1">
            {canEdit && (
              <Button
                type="text"
                size="small"
                icon={<EditOutlined />}
                onClick={e => {
                  e.stopPropagation()
                  handleEdit()
                }}
                aria-label={`Edit ${assistant.name}`}
              />
            )}
            {canDelete && (
              <Popconfirm
                title="Delete Assistant"
                description={`Are you sure you want to delete "${assistant.name}"?`}
                okText="Delete"
                cancelText="Cancel"
                okButtonProps={{ danger: true }}
                onConfirm={onDelete}
                onPopupClick={e => e.stopPropagation()}
              >
                <Button
                  type="text"
                  size="small"
                  danger
                  icon={<DeleteOutlined />}
                  onClick={e => e.stopPropagation()}
                  aria-label={`Delete ${assistant.name}`}
                />
              </Popconfirm>
            )}
          </div>
        )}
      </Flex>
    </Card>
  )
}

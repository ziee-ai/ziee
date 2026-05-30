import { useState } from 'react'
import { App, Badge, Button, Checkbox, Descriptions, Flex, Popconfirm, Space, Tag, Typography } from 'antd'
import {
  CheckCircleOutlined,
  DeleteOutlined,
  StarOutlined
} from '@ant-design/icons'

const { Text } = Typography
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type RuntimeVersionResponse } from '@/api-client/types'

interface Props {
  version: RuntimeVersionResponse
}

export function RuntimeVersionCard({ version }: Props) {
  const { settingDefault, deleting } = Stores.RuntimeVersion

  const isSettingDefault = settingDefault.get(version.id) || false
  const isDeleting = deleting.get(version.id) || false

  const canUpdate = usePermission(Permissions.RuntimeVersionUpdate)
  const canDelete = usePermission(Permissions.RuntimeVersionDelete)

  const [removeBinary, setRemoveBinary] = useState(false)
  const { message } = App.useApp()

  const handleSetDefault = async () => {
    try {
      await Stores.RuntimeVersion.setDefaultVersion(version.id)
    } catch (error) {
      // Error already handled in store
    }
  }

  const handleDelete = async () => {
    try {
      await Stores.RuntimeVersion.deleteVersion(version.id, removeBinary)
    } catch (error) {
      // Surface the in-use guard (409) reason, e.g. which models/providers
      // still depend on this version.
      message.error(
        error instanceof Error ? error.message : 'Failed to delete version'
      )
    }
  }

  return (
    <Flex vertical gap="small">
      <Space>
        <Badge
          status={version.is_system_default ? 'success' : 'default'}
          text={
            version.is_system_default ? (
              <Text strong>Version {version.version}</Text>
            ) : (
              <Text>Version {version.version}</Text>
            )
          }
        />
        {version.is_system_default && (
          <Tag icon={<CheckCircleOutlined />} color="success">
            Default
          </Tag>
        )}
      </Space>

      <Descriptions size="small" column={2}>
        <Descriptions.Item label="Platform">
          {version.platform}
        </Descriptions.Item>
        <Descriptions.Item label="Architecture">
          {version.arch}
        </Descriptions.Item>
        <Descriptions.Item label="Backend">
          {version.backend.toUpperCase()}
        </Descriptions.Item>
        <Descriptions.Item label="Installed">
          {new Date(version.created_at).toLocaleString()}
        </Descriptions.Item>
      </Descriptions>

      <Space>
        {canUpdate && !version.is_system_default && (
          <Button
            icon={<StarOutlined />}
            loading={isSettingDefault}
            onClick={handleSetDefault}
          >
            Set as Default
          </Button>
        )}

        {canDelete && (
          <Popconfirm
            title="Delete Runtime Version"
            description={
              <Flex vertical gap="small">
                <Text>
                  Are you sure you want to delete version {version.version}?
                </Text>
                {version.is_system_default && (
                  <Text type="danger">
                    Warning: This is the default version.
                  </Text>
                )}
                <Checkbox
                  checked={removeBinary}
                  onChange={e => setRemoveBinary(e.target.checked)}
                >
                  Also remove cached files from disk
                </Checkbox>
              </Flex>
            }
            onConfirm={handleDelete}
            okText="Delete"
            okButtonProps={{ danger: true }}
          >
            <Button
              danger
              icon={<DeleteOutlined />}
              loading={isDeleting}
            >
              Delete
            </Button>
          </Popconfirm>
        )}
      </Space>
    </Flex>
  )
}

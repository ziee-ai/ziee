import { Badge, Button, Descriptions, Popconfirm, Space, Tag } from 'antd'
import {
  CheckCircleOutlined,
  DeleteOutlined,
  StarOutlined
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import type { RuntimeVersionResponse } from '@/api-client/types'

interface Props {
  version: RuntimeVersionResponse
}

export function RuntimeVersionCard({ version }: Props) {
  const { settingDefault, deleting } = Stores.RuntimeVersion

  const isSettingDefault = settingDefault.get(version.id) || false
  const isDeleting = deleting.get(version.id) || false

  const canUpdate = usePermission('llm_local_runtime::update')
  const canDelete = usePermission('llm_local_runtime::delete')

  const handleSetDefault = async () => {
    try {
      await Stores.RuntimeVersion.setDefaultVersion(version.id)
    } catch (error) {
      // Error already handled in store
    }
  }

  const handleDelete = async () => {
    try {
      await Stores.RuntimeVersion.deleteVersion(version.id)
    } catch (error) {
      // Error already handled in store
    }
  }

  return (
    <div>
      <Space style={{ marginBottom: 8 }}>
        <Badge
          status={version.is_system_default ? 'success' : 'default'}
          text={
            <span style={{ fontWeight: version.is_system_default ? 600 : 400 }}>
              Version {version.version}
            </span>
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

      <Space style={{ marginTop: 12 }}>
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
              <>
                Are you sure you want to delete version {version.version}?
                {version.is_system_default && (
                  <div style={{ color: '#ff4d4f', marginTop: 8 }}>
                    Warning: This is the default version.
                  </div>
                )}
              </>
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
    </div>
  )
}

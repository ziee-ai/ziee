import { Alert, Button, Card, Space, Tag } from 'antd'
import { CloudSyncOutlined, ReloadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { RuntimeEngine } from '../types'

interface Props {
  engine: RuntimeEngine
}

export function RuntimeUpdateChecker({ engine }: Props) {
  const { updateChecks, checking } = Stores.RuntimeUpdate
  const { openDrawer } = Stores.RuntimeDownloadDrawer

  const updateCheck = updateChecks.get(engine)
  const isChecking = checking.get(engine) || false

  const handleCheck = async () => {
    try {
      await Stores.RuntimeUpdate.checkForUpdates(engine)
    } catch (error) {
      // Error already handled in store
    }
  }

  return (
    <Card
      size="small"
      style={{ marginBottom: 16 }}
      extra={
        <Button
          icon={<ReloadOutlined />}
          loading={isChecking}
          onClick={handleCheck}
        >
          Check for Updates
        </Button>
      }
    >
      {!updateCheck ? (
        <Alert
          message="Click 'Check for Updates' to see available versions from GitHub"
          type="info"
          showIcon
        />
      ) : updateCheck.has_updates ? (
        <Alert
          message={`Updates available for ${engine}`}
          description={
            <Space direction="vertical" style={{ width: '100%' }}>
              <div>
                Current: <Tag>{updateCheck.current_version || 'None'}</Tag>
                Latest: <Tag color="green">{updateCheck.latest_version}</Tag>
              </div>
              <div>
                <strong>Available versions:</strong>
                <div style={{ marginTop: 8 }}>
                  {updateCheck.available_versions.slice(0, 5).map(version => (
                    <Tag key={version}>{version}</Tag>
                  ))}
                  {updateCheck.available_versions.length > 5 && (
                    <Tag>+{updateCheck.available_versions.length - 5} more</Tag>
                  )}
                </div>
              </div>
              <Button
                type="primary"
                icon={<CloudSyncOutlined />}
                onClick={() => openDrawer(engine)}
              >
                Download Update
              </Button>
            </Space>
          }
          type="warning"
          showIcon
        />
      ) : (
        <Alert
          message={`${engine} is up to date`}
          description={`Current version: ${updateCheck.current_version}`}
          type="success"
          showIcon
        />
      )}
    </Card>
  )
}

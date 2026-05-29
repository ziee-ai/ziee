import { Alert, Button, Card, Flex, Tag } from 'antd'
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
          title="Click 'Check for Updates' to see available versions from GitHub"
          type="info"
          showIcon
        />
      ) : updateCheck.has_updates ? (
        <Alert
          title={`Updates available for ${engine}`}
          description={
            <Flex vertical gap="small" style={{ width: '100%' }}>
              <div>
                Current: <Tag>{updateCheck.current_version || 'None'}</Tag>
                Latest: <Tag color="green">{updateCheck.latest_version}</Tag>
              </div>
              {(() => {
                // Only show releases whose binary is published for this host;
                // build-pending tags are not surfaced.
                const ready = updateCheck.versions.filter(v => v.binary_ready)
                return (
                  <div>
                    <strong>
                      Releases ({updateCheck.platform}/{updateCheck.arch}):
                    </strong>
                    <div style={{ marginTop: 8 }}>
                      {ready.slice(0, 5).map(v => (
                        <Tag key={v.version} color={v.installed ? 'green' : 'blue'}>
                          {v.version}
                          {v.installed ? ' (installed)' : ''}
                        </Tag>
                      ))}
                      {ready.length > 5 && (
                        <Tag>+{ready.length - 5} more</Tag>
                      )}
                    </div>
                  </div>
                )
              })()}
              <Button
                type="primary"
                icon={<CloudSyncOutlined />}
                onClick={() => openDrawer(engine)}
              >
                Download Update
              </Button>
            </Flex>
          }
          type="warning"
          showIcon
        />
      ) : (
        <Alert
          title={`${engine} is up to date`}
          description={`Current version: ${updateCheck.current_version}`}
          type="success"
          showIcon
        />
      )}
    </Card>
  )
}

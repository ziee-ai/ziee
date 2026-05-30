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
      title="Updates"
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
            <Flex vertical gap="small">
              <div>
                Current: <Tag>{updateCheck.current_version || 'None'}</Tag>
                Latest: <Tag color="green">{updateCheck.latest_version}</Tag>
              </div>
              {(() => {
                // Only show releases whose binary is published for this host;
                // build-pending tags are not surfaced.
                const ready = updateCheck.versions.filter(v => v.binary_ready)
                return (
                  <Flex vertical gap="small">
                    <strong>
                      Releases ({updateCheck.platform}/{updateCheck.arch}):
                    </strong>
                    <div>
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
                  </Flex>
                )
              })()}
              <Flex justify="end">
                <Button
                  type="primary"
                  icon={<CloudSyncOutlined />}
                  onClick={() => openDrawer(engine)}
                >
                  Download Update
                </Button>
              </Flex>
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

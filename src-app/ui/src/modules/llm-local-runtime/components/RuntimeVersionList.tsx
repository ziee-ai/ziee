import React from 'react'
import { Button, Card, Empty, Spin, Space, Divider, Typography, message } from 'antd'
import { DownloadOutlined } from '@ant-design/icons'

const { Text } = Typography
import { Stores } from '@/core/stores'
import { Can, usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import type { RuntimeEngine } from '../types'
import { RuntimeVersionCard } from './RuntimeVersionCard'

interface Props {
  engine: RuntimeEngine
}

export function RuntimeVersionList({ engine }: Props) {
  const { versions, loading, error } = Stores.RuntimeVersion
  const { openDrawer } = Stores.RuntimeDownloadDrawer

  const canCreate = usePermission(Permissions.RuntimeVersionCreate)
  const engineVersions = versions.filter(v => v.engine === engine)

  // Show error message
  React.useEffect(() => {
    if (error) {
      message.error(error)
      Stores.RuntimeVersion.clearError()
    }
  }, [error])

  if (loading && !versions.length) {
    return (
      <Card>
        <Spin />
      </Card>
    )
  }

  if (!engineVersions.length) {
    // Distinguish "no versions across any engine" (first-run) from
    // "this engine has none, but the user has installed others"
    // (second-engine onboarding). Different copy + different CTA
    // emphasis. Matches the empty-state pattern peer modules (mcp,
    // assistant) use to avoid the same generic "No X" blank screen
    // for every empty-list reason.
    const hasOtherEngineVersions = versions.length > 0
    return (
      <Card title="Installed Versions">
        <Empty
          description={
            hasOtherEngineVersions
              ? `No ${engine} versions installed yet — other engines have versions.`
              : `No engine versions installed yet. Download one to get started.`
          }
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        >
          {canCreate && (
            <Button
              type="primary"
              icon={<DownloadOutlined />}
              onClick={() => openDrawer(engine)}
            >
              Download {engine} Version
            </Button>
          )}
        </Empty>
      </Card>
    )
  }

  return (
    <Card
      title={
        <Space>
          <span>Installed Versions</span>
          <Text type="secondary">({engineVersions.length})</Text>
        </Space>
      }
      extra={
        <Can permission={Permissions.RuntimeVersionCreate}>
          <Button
            type="primary"
            icon={<DownloadOutlined />}
            onClick={() => openDrawer(engine)}
          >
            Download Version
          </Button>
        </Can>
      }
    >
      {engineVersions.map((version, index) => (
        <React.Fragment key={version.id}>
          {index > 0 && <Divider />}
          <RuntimeVersionCard version={version} />
        </React.Fragment>
      ))}
    </Card>
  )
}

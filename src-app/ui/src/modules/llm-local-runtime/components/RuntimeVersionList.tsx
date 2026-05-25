import React from 'react'
import { Button, Card, Empty, Spin, Space, Divider, message } from 'antd'
import { DownloadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { Can, usePermission } from '@/core/permissions'
import type { RuntimeEngine } from '../types'
import { RuntimeVersionCard } from './RuntimeVersionCard'

interface Props {
  engine: RuntimeEngine
}

export function RuntimeVersionList({ engine }: Props) {
  const { versions, loading, error } = Stores.RuntimeVersion
  const { openDrawer } = Stores.RuntimeDownloadDrawer

  const canCreate = usePermission('llm_local_runtime::create')
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
        <Spin tip="Loading versions..." />
      </Card>
    )
  }

  if (!engineVersions.length) {
    return (
      <Card>
        <Empty
          description={`No ${engine} versions installed`}
          image={Empty.PRESENTED_IMAGE_SIMPLE}
        >
          {canCreate && (
            <Button
              type="primary"
              icon={<DownloadOutlined />}
              onClick={() => openDrawer(engine)}
            >
              Download Version
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
          <span style={{ color: '#999', fontWeight: 'normal' }}>
            ({engineVersions.length})
          </span>
        </Space>
      }
      extra={
        <Can permission="llm_local_runtime::create">
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

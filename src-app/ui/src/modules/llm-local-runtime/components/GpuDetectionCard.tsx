import { Button, Card, Flex, Space, Spin, Tag, Typography } from 'antd'
import { ThunderboltOutlined, DownloadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import type { RuntimeEngine } from '../types'

const { Text } = Typography

const BACKEND_LABEL: Record<string, string> = {
  cpu: 'CPU',
  cuda: 'NVIDIA CUDA',
  metal: 'Apple Metal',
  rocm: 'AMD ROCm',
  vulkan: 'Vulkan',
  opencl: 'OpenCL',
}

/**
 * GPU detection card (P3). Surfaces which engine backend(s) the host
 * supports + the recommended pick, with a "Download recommended"
 * shortcut that pre-opens the runtime download drawer for the given
 * engine. The drawer reads the recommended backend/arch from this
 * same store on open.
 */
export function GpuDetectionCard({ engine }: { engine: RuntimeEngine }) {
  const { gpu, loadingGpu } = Stores.RuntimeConfig
  const { openDrawer } = Stores.RuntimeDownloadDrawer

  if (loadingGpu && !gpu) {
    return (
      <Card size="small">
        <Spin />
      </Card>
    )
  }

  if (!gpu) {
    return null
  }

  const recommendedLabel = BACKEND_LABEL[gpu.recommended] ?? gpu.recommended

  return (
    <Card
      size="small"
      title={
        <Space>
          <ThunderboltOutlined />
          <span>Hardware acceleration</span>
        </Space>
      }
    >
      <Flex vertical gap="small" style={{ width: '100%' }}>
        <div>
          <Text type="secondary">Detected platform: </Text>
          <Text strong>
            {gpu.platform}/{gpu.arch}
          </Text>
        </div>
        <div>
          <Text type="secondary">Available backends: </Text>
          {gpu.available.map(b => (
            <Tag
              key={b}
              color={b === gpu.recommended ? 'green' : undefined}
            >
              {BACKEND_LABEL[b] ?? b}
            </Tag>
          ))}
        </div>
        <Text>
          Recommended backend for this host: <Text strong>{recommendedLabel}</Text>
        </Text>
        <Can permission={Permissions.RuntimeVersionCreate}>
          <Button
            type="primary"
            icon={<DownloadOutlined />}
            onClick={() => openDrawer(engine)}
          >
            Download recommended ({recommendedLabel})
          </Button>
        </Can>
      </Flex>
    </Card>
  )
}

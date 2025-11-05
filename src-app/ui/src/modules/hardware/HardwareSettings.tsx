import {
  Alert,
  App,
  Button,
  Card,
  Progress,
  Spin,
  Statistic,
  Tag,
  Typography,
} from 'antd'
import { useEffect } from 'react'
import { MdOutlineMonitorHeart } from 'react-icons/md'
import { Stores } from '@/core/stores'
import { disconnectHardwareUsage, subscribeToHardwareUsage } from './store'
import { formatBytes } from './utils/formatBytes'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'

const { Text } = Typography

export default function HardwareSettings() {
  const { message } = App.useApp()

  // Hardware store state
  const {
    hardwareInfo,
    hardwareLoading,
    hardwareError,
    currentUsage,
    usageLoading,
    usageError,
    sseConnected,
    sseError,
  } = Stores.Hardware

  // Initialize hardware monitoring on component mount
  useEffect(() => {
    // TODO: Check hardware::monitor permission before auto-connecting
    subscribeToHardwareUsage().catch(console.error)

    // Cleanup on component unmount
    return () => {
      disconnectHardwareUsage()
    }
  }, [])

  // Show errors
  useEffect(() => {
    if (hardwareError) {
      message.error(`Hardware Error: ${hardwareError}`)
    }
    if (usageError) {
      message.error(`Usage Monitoring Error: ${usageError}`)
    }
    if (sseError) {
      message.error(`Connection Error: ${sseError}`)
    }
  }, [hardwareError, usageError, sseError, message])

  if (hardwareLoading) {
    return (
      <SettingsPageContainer title="Hardware">
        <div style={{ textAlign: 'center', padding: '50px' }}>
          <Spin size="large" />
          <Text
            type="secondary"
            style={{ display: 'block', marginTop: '16px' }}
          >
            Loading hardware information...
          </Text>
        </div>
      </SettingsPageContainer>
    )
  }

  if (hardwareError && !hardwareInfo) {
    return (
      <SettingsPageContainer title="Hardware">
        <Alert
          message="Hardware Information Unavailable"
          description={hardwareError}
          type="error"
          showIcon
        />
      </SettingsPageContainer>
    )
  }

  const renderOperatingSystemCard = () => (
    <Card title="Operating System">
      <div className="flex flex-wrap gap-6">
        <Statistic
          title="Name"
          value={hardwareInfo?.operating_system.name || 'Unknown'}
        />
        <Statistic
          title="Version"
          value={hardwareInfo?.operating_system.version || 'Unknown'}
        />
        <Statistic
          title="Architecture"
          value={hardwareInfo?.operating_system.architecture || 'Unknown'}
        />
        {hardwareInfo?.operating_system.kernel_version && (
          <Statistic
            title="Kernel"
            value={hardwareInfo.operating_system.kernel_version}
          />
        )}
      </div>
    </Card>
  )

  const renderCPUCard = () => (
    <Card title="CPU">
      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        <div className="flex flex-wrap gap-6">
          <Statistic
            title="Model"
            value={hardwareInfo?.cpu.model || 'Unknown'}
          />
          <Statistic
            title="Architecture"
            value={hardwareInfo?.cpu.architecture || 'Unknown'}
          />
          <Statistic title="Cores" value={hardwareInfo?.cpu.cores || 0} />
          {hardwareInfo?.cpu.threads && (
            <Statistic title="Threads" value={hardwareInfo.cpu.threads} />
          )}
          {hardwareInfo?.cpu.base_frequency && (
            <Statistic
              title="Base Frequency"
              value={`${hardwareInfo.cpu.base_frequency} MHz`}
            />
          )}
          {hardwareInfo?.cpu.max_frequency && (
            <Statistic
              title="Max Frequency"
              value={`${hardwareInfo.cpu.max_frequency} MHz`}
            />
          )}
        </div>
        {currentUsage && (
          <div>
            <Text strong>CPU Usage</Text>
            <Progress
              percent={currentUsage.cpu.usage_percentage}
              status={
                currentUsage.cpu.usage_percentage > 90 ? 'exception' : 'active'
              }
              format={percent =>
                `${percent != null ? percent.toFixed(1) : '0.0'}%`
              }
            />
            <div className="flex gap-3" style={{ marginTop: '8px' }}>
              {currentUsage.cpu.temperature && (
                <Text type="secondary" style={{ fontSize: '12px' }}>
                  Temperature: {currentUsage.cpu.temperature}°C
                </Text>
              )}
              {currentUsage.cpu.frequency && (
                <Text type="secondary" style={{ fontSize: '12px' }}>
                  Current: {currentUsage.cpu.frequency} MHz
                </Text>
              )}
            </div>
          </div>
        )}
      </div>
    </Card>
  )

  const renderMemoryCard = () => (
    <Card title="Memory">
      <div style={{ display: 'flex', flexDirection: 'column', gap: '16px' }}>
        <div className="flex flex-wrap gap-6">
          <div>
            <Text
              type="secondary"
              style={{ fontSize: '12px', display: 'block' }}
            >
              Total RAM
            </Text>
            <div style={{ fontSize: '24px', fontWeight: 600 }}>
              {formatBytes(hardwareInfo?.memory.total_ram || 0)}
            </div>
          </div>
          {hardwareInfo?.memory.total_swap !== undefined &&
            hardwareInfo.memory.total_swap > 0 && (
              <div>
                <Text
                  type="secondary"
                  style={{ fontSize: '12px', display: 'block' }}
                >
                  Total Swap
                </Text>
                <div style={{ fontSize: '24px', fontWeight: 600 }}>
                  {formatBytes(hardwareInfo.memory.total_swap)}
                </div>
              </div>
            )}
        </div>
        {currentUsage && (
          <div>
            <Text strong>Memory Usage</Text>
            <Progress
              percent={currentUsage.memory.usage_percentage}
              status={
                currentUsage.memory.usage_percentage > 90
                  ? 'exception'
                  : 'active'
              }
              format={percent =>
                `${percent != null ? percent.toFixed(1) : '0.0'}%`
              }
            />
            <div className="flex gap-3" style={{ marginTop: '8px' }}>
              <Text type="secondary" style={{ fontSize: '12px' }}>
                Used: {formatBytes(currentUsage.memory.used_ram)}
              </Text>
              <Text type="secondary" style={{ fontSize: '12px' }}>
                Available: {formatBytes(currentUsage.memory.available_ram)}
              </Text>
            </div>
          </div>
        )}
      </div>
    </Card>
  )

  const renderGPUCards = () => {
    if (!hardwareInfo?.gpu_devices || hardwareInfo.gpu_devices.length === 0) {
      return (
        <Card title="GPU">
          <Text type="secondary">No GPU devices detected</Text>
        </Card>
      )
    }

    return hardwareInfo.gpu_devices.map((gpu, index) => {
      // Match GPU usage by device ID (more reliable than name matching)
      const gpuUsage =
        currentUsage?.gpu_devices.find(
          usage => usage.device_id === gpu.device_id,
        ) ||
        // Fallback: if only one GPU in each array, match them
        (hardwareInfo.gpu_devices.length === 1 &&
        currentUsage?.gpu_devices.length === 1
          ? currentUsage.gpu_devices[0]
          : undefined)

      return (
        <Card key={index} title={gpu.name}>
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: '16px',
            }}
          >
            <div className="flex flex-wrap gap-6">
              <div>
                <Text
                  type="secondary"
                  style={{ fontSize: '12px', display: 'block' }}
                >
                  Vendor
                </Text>
                <div style={{ fontSize: '24px', fontWeight: 600 }}>
                  {gpu.vendor}
                </div>
              </div>
              {gpu.memory ? (
                <div>
                  <Text
                    type="secondary"
                    style={{ fontSize: '12px', display: 'block' }}
                  >
                    {gpu.vendor?.includes('Apple')
                      ? 'Dedicated VRAM'
                      : 'Memory'}
                  </Text>
                  <div style={{ fontSize: '24px', fontWeight: 600 }}>
                    {formatBytes(gpu.memory)}
                  </div>
                </div>
              ) : gpu.vendor?.includes('Apple') ? (
                <div>
                  <Text
                    type="secondary"
                    style={{ fontSize: '12px', display: 'block' }}
                  >
                    Memory Architecture
                  </Text>
                  <div style={{ fontSize: '24px', fontWeight: 600 }}>
                    Unified Memory
                  </div>
                </div>
              ) : null}
              {gpu.driver_version && (
                <div>
                  <Text
                    type="secondary"
                    style={{ fontSize: '12px', display: 'block' }}
                  >
                    Driver
                  </Text>
                  <div style={{ fontSize: '24px', fontWeight: 600 }}>
                    {gpu.driver_version}
                  </div>
                </div>
              )}
              {gpu.vendor?.includes('Apple') && hardwareInfo?.memory && (
                <div>
                  <Text
                    type="secondary"
                    style={{ fontSize: '12px', display: 'block' }}
                  >
                    Shared System Memory
                  </Text>
                  <div style={{ fontSize: '24px', fontWeight: 600 }}>
                    {formatBytes(hardwareInfo.memory.total_ram)}
                  </div>
                </div>
              )}
            </div>

            <div>
              <Text strong style={{ marginBottom: '8px', display: 'block' }}>
                Compute Support
              </Text>
              <div className="flex flex-wrap gap-1">
                <Tag
                  color={
                    gpu.compute_capabilities.cuda_support ? 'green' : 'default'
                  }
                >
                  CUDA {gpu.compute_capabilities.cuda_support ? '✓' : '✗'}
                  {gpu.compute_capabilities.cuda_version &&
                    ` (${gpu.compute_capabilities.cuda_version})`}
                </Tag>
                <Tag
                  color={
                    gpu.compute_capabilities.metal_support ? 'green' : 'default'
                  }
                >
                  Metal {gpu.compute_capabilities.metal_support ? '✓' : '✗'}
                </Tag>
                <Tag
                  color={
                    gpu.compute_capabilities.opencl_support
                      ? 'green'
                      : 'default'
                  }
                >
                  OpenCL {gpu.compute_capabilities.opencl_support ? '✓' : '✗'}
                </Tag>
                {gpu.compute_capabilities.vulkan_support !== undefined && (
                  <Tag
                    color={
                      gpu.compute_capabilities.vulkan_support
                        ? 'green'
                        : 'default'
                    }
                  >
                    Vulkan {gpu.compute_capabilities.vulkan_support ? '✓' : '✗'}
                  </Tag>
                )}
              </div>
            </div>

            {gpuUsage && (
              <div>
                {gpuUsage.utilization_percentage !== undefined && (
                  <div style={{ marginBottom: '12px' }}>
                    <Text strong>GPU Utilization</Text>
                    <Progress
                      percent={gpuUsage.utilization_percentage}
                      status={
                        gpuUsage.utilization_percentage > 90
                          ? 'exception'
                          : 'active'
                      }
                      format={percent =>
                        `${percent != null ? percent.toFixed(1) : '0.0'}%`
                      }
                    />
                  </div>
                )}

                {(gpuUsage.memory_usage_percentage !== undefined ||
                  (gpuUsage.memory_used !== undefined &&
                    gpuUsage.memory_total !== undefined)) && (
                  <div style={{ marginBottom: '12px' }}>
                    <Text strong>
                      {gpu.vendor?.includes('Apple')
                        ? 'System Memory Usage'
                        : 'GPU Memory'}
                    </Text>
                    {gpuUsage.memory_usage_percentage !== undefined ? (
                      <Progress
                        percent={gpuUsage.memory_usage_percentage}
                        status={
                          gpuUsage.memory_usage_percentage > 90
                            ? 'exception'
                            : 'active'
                        }
                        format={percent =>
                          `${percent != null ? percent.toFixed(1) : '0.0'}%`
                        }
                      />
                    ) : gpuUsage.memory_used !== undefined &&
                      gpuUsage.memory_total !== undefined ? (
                      <Progress
                        percent={
                          (gpuUsage.memory_used / gpuUsage.memory_total) * 100
                        }
                        status={
                          (gpuUsage.memory_used / gpuUsage.memory_total) * 100 >
                          90
                            ? 'exception'
                            : 'active'
                        }
                        format={percent =>
                          `${percent != null ? percent.toFixed(1) : '0.0'}%`
                        }
                      />
                    ) : null}

                    {gpuUsage.memory_used !== undefined &&
                      gpuUsage.memory_total !== undefined && (
                        <div style={{ marginTop: '4px' }}>
                          <Text
                            type="secondary"
                            style={{ fontSize: '12px', display: 'block' }}
                          >
                            {gpu.vendor?.includes('Apple')
                              ? 'GPU Memory Used: '
                              : 'Used: '}
                            {formatBytes(gpuUsage.memory_used)}
                            {gpu.vendor?.includes('Apple')
                              ? ` of ${formatBytes(gpuUsage.memory_total)} total system memory`
                              : ` / ${formatBytes(gpuUsage.memory_total)}`}
                          </Text>
                          {gpu.vendor?.includes('Apple') && (
                            <Text
                              type="secondary"
                              style={{
                                fontSize: '11px',
                                display: 'block',
                                fontStyle: 'italic',
                              }}
                            >
                              Apple Silicon uses unified memory architecture
                            </Text>
                          )}
                        </div>
                      )}
                  </div>
                )}

                {/* Real-time GPU Statistics */}
                {gpuUsage &&
                  (gpuUsage.utilization_percentage !== undefined ||
                    gpuUsage.memory_used !== undefined ||
                    gpuUsage.temperature !== undefined ||
                    gpuUsage.power_usage !== undefined) && (
                    <div style={{ marginBottom: '12px' }}>
                      <Text
                        strong
                        style={{ display: 'block', marginBottom: '8px' }}
                      >
                        Real-time Statistics
                      </Text>
                      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                        {gpuUsage.utilization_percentage !== undefined && (
                          <div>
                            <Text type="secondary" style={{ fontSize: '11px' }}>
                              GPU Usage
                            </Text>
                            <div
                              style={{ fontSize: '14px', fontWeight: 'bold' }}
                            >
                              {gpuUsage.utilization_percentage != null
                                ? gpuUsage.utilization_percentage.toFixed(1)
                                : '0.0'}
                              %
                            </div>
                          </div>
                        )}
                        {gpuUsage.memory_usage_percentage !== undefined && (
                          <div>
                            <Text type="secondary" style={{ fontSize: '11px' }}>
                              Memory Usage
                            </Text>
                            <div
                              style={{ fontSize: '14px', fontWeight: 'bold' }}
                            >
                              {gpuUsage.memory_usage_percentage != null
                                ? gpuUsage.memory_usage_percentage.toFixed(1)
                                : '0.0'}
                              %
                            </div>
                          </div>
                        )}
                        {gpuUsage.memory_used !== undefined && (
                          <div>
                            <Text type="secondary" style={{ fontSize: '11px' }}>
                              Memory Used
                            </Text>
                            <div
                              style={{ fontSize: '14px', fontWeight: 'bold' }}
                            >
                              {formatBytes(gpuUsage.memory_used)}
                            </div>
                          </div>
                        )}
                        {gpuUsage.temperature !== undefined && (
                          <div>
                            <Text type="secondary" style={{ fontSize: '11px' }}>
                              Temperature
                            </Text>
                            <div
                              style={{ fontSize: '14px', fontWeight: 'bold' }}
                            >
                              {gpuUsage.temperature}°C
                            </div>
                          </div>
                        )}
                        {gpuUsage.power_usage !== undefined && (
                          <div>
                            <Text type="secondary" style={{ fontSize: '11px' }}>
                              Power Draw
                            </Text>
                            <div
                              style={{ fontSize: '14px', fontWeight: 'bold' }}
                            >
                              {gpuUsage.power_usage != null
                                ? gpuUsage.power_usage.toFixed(1)
                                : '0.0'}
                              W
                            </div>
                          </div>
                        )}
                      </div>
                    </div>
                  )}

                <div className="flex gap-3">
                  {gpuUsage.temperature !== undefined && (
                    <Text type="secondary" style={{ fontSize: '12px' }}>
                      Temperature: {gpuUsage.temperature}°C
                    </Text>
                  )}
                  {gpuUsage.power_usage !== undefined && (
                    <Text type="secondary" style={{ fontSize: '12px' }}>
                      Power: {gpuUsage.power_usage}W
                    </Text>
                  )}
                </div>
              </div>
            )}
          </div>
        </Card>
      )
    })
  }

  const handleManualConnect = async () => {
    try {
      await subscribeToHardwareUsage()
      message.success('Connecting to hardware monitoring...')
    } catch (_error) {
      message.error('Failed to connect to hardware monitoring')
    }
  }

  const handleOpenMonitorPopup = async () => {
    try {
      // Use browser popup for web app
      const popup = window.open(
        window.location.origin + '/hardware-monitor',
        'hardware-monitor', // Using same name will focus existing popup
        'width=800,height=600,scrollbars=yes,resizable=yes,menubar=no,toolbar=no',
      )
      if (popup) {
        popup.focus()
      } else {
        message.error('Please allow popups for this website')
      }
    } catch (error) {
      console.error('Error opening hardware monitor:', error)
      message.error('Failed to open hardware monitor')
    }
  }

  const renderConnectionStatus = () => (
    <Card
      style={{
        display: sseConnected ? 'none' : 'block',
      }}
    >
      <div className="flex items-center gap-3 flex-wrap">
        <div className="flex gap-3 flex-wrap">
          <Text strong>Real-time Monitoring:</Text>
          <Tag color={sseConnected ? 'green' : 'red'}>
            {sseConnected ? 'Connected' : 'Disconnected'}
          </Tag>
        </div>
        {!sseConnected && !usageLoading && (
          <Button type="primary" onClick={handleManualConnect}>
            Connect
          </Button>
        )}
        {usageLoading && (
          <div className="flex items-center gap-2">
            <Spin />
            <Text type="secondary">Connecting...</Text>
          </div>
        )}
        {currentUsage && (
          <Text type="secondary" style={{ fontSize: '12px' }}>
            Last update: {new Date(currentUsage.timestamp).toLocaleTimeString()}
          </Text>
        )}
      </div>
    </Card>
  )

  const titleWithButton = (
    <div className="flex items-center justify-between w-full">
      <span>Hardware</span>
      <Button icon={<MdOutlineMonitorHeart />} onClick={handleOpenMonitorPopup}>
        Monitor
      </Button>
    </div>
  )

  return (
    <SettingsPageContainer title={titleWithButton}>
      {renderConnectionStatus()}

      {renderOperatingSystemCard()}
      {renderCPUCard()}
      {renderMemoryCard()}
      {renderGPUCards()}
    </SettingsPageContainer>
  )
}

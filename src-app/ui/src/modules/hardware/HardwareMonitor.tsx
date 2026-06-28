import { Alert, Button, Card, Progress, Spin, Tag, Text, message } from '@/components/ui'
import { Loading } from '@/core/components/Loading'
import { useEffect } from 'react'
import { Stores } from '@/core/stores'
import { DivScrollY } from '@/components/common/DivScrollY'
import { formatBytes } from '@/modules/hardware/utils/formatBytes'

export function HardwareMonitor() {
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
    // Load hardware info first, then start monitoring
    Stores.Hardware.subscribeToHardwareUsage().catch(console.error)

    // Cleanup on component unmount
    return () => {
      Stores.Hardware.disconnectHardwareUsage()
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
  }, [hardwareError, usageError, sseError])

  const handleManualConnect = async () => {
    try {
      await Stores.Hardware.subscribeToHardwareUsage()
      message.success('Connecting to hardware monitoring...')
    } catch (_error) {
      message.error('Failed to connect to hardware monitoring')
    }
  }

  const renderConnectionStatus = () => (
    <Card
      data-testid="hardware-connection-card"
      className={sseConnected ? 'hidden' : 'block'}
    >
      <div className={'flex flex-wrap justify-between gap-3'}>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <Text strong>Real-time Monitoring:</Text>
            <Tag data-testid="hardware-connection-status-tag" tone={sseConnected ? 'success' : 'error'}>
              {sseConnected ? 'Connected' : 'Disconnected'}
            </Tag>
            {usageLoading && (
              <div className="flex items-center gap-2">
                <Spin label="Connecting..." />
                <Text type="secondary">Connecting...</Text>
              </div>
            )}
          </div>
          {!sseConnected && !usageLoading && (
            <Button data-testid="hardware-connect-btn" variant="default" onClick={handleManualConnect}>
              Connect
            </Button>
          )}
        </div>
        {currentUsage && (
          <Text type="secondary" className="text-xs">
            Last update: {new Date(currentUsage.timestamp).toLocaleTimeString()}
          </Text>
        )}
      </div>
    </Card>
  )

  const renderCPUUsage = () => {
    if (!currentUsage) return null

    return (
      <Card title="CPU Usage" data-testid="hardware-cpu-card">
        <Progress
          data-testid="hardware-cpu-progress"
          value={currentUsage.cpu.usage_percentage}
          tone={currentUsage.cpu.usage_percentage > 90 ? 'error' : 'primary'}
          aria-label="CPU usage"
          format={percent => `${percent != null ? percent.toFixed(1) : '0.0'}%`}
        />
        <div className="flex gap-3 mt-2">
          {currentUsage.cpu.temperature && (
            <Text type="secondary" className="text-xs">
              Temperature: {currentUsage.cpu.temperature}°C
            </Text>
          )}
          {currentUsage.cpu.frequency && (
            <Text type="secondary" className="text-xs">
              Frequency: {currentUsage.cpu.frequency} MHz
            </Text>
          )}
        </div>
      </Card>
    )
  }

  const renderMemoryUsage = () => {
    if (!currentUsage) return null

    return (
      <Card title="Memory Usage" data-testid="hardware-memory-card">
        <Progress
          data-testid="hardware-memory-progress"
          value={currentUsage.memory.usage_percentage}
          tone={currentUsage.memory.usage_percentage > 90 ? 'error' : 'primary'}
          aria-label="Memory usage"
          format={percent => `${percent != null ? percent.toFixed(1) : '0.0'}%`}
        />
        <div className="flex gap-3 mt-2">
          <Text type="secondary" className="text-xs">
            Used: {formatBytes(currentUsage.memory.used_ram)}
          </Text>
          <Text type="secondary" className="text-xs">
            Available: {formatBytes(currentUsage.memory.available_ram)}
          </Text>
        </div>
      </Card>
    )
  }

  if (hardwareLoading) {
    return (
      <div className="p-3">
        <Loading tip="Loading hardware monitor..." />
      </div>
    )
  }

  if (hardwareError && !hardwareInfo) {
    return (
      <div className="p-3">
        <Alert
          data-testid="hardware-unavailable-alert"
          title="Hardware Monitor Unavailable"
          description={hardwareError}
          tone="error"
        />
      </div>
    )
  }

  return (
    <DivScrollY className="h-full w-full flex-col">
      <div className="p-3 max-w-4xl mx-auto w-full">
        <div className="flex flex-col gap-3">
          {renderConnectionStatus()}

        {currentUsage ? (
          <>
            {/* CPU and Memory Usage - First Row */}
            <div className="flex gap-3 flex-wrap">
              <div className="flex-1 min-w-80">{renderCPUUsage()}</div>
              <div className="flex-1 min-w-80">{renderMemoryUsage()}</div>
            </div>

            {/* GPU Usage Cards - Arranged with wrapping support */}
            <div className="flex gap-3 flex-wrap">
              {!currentUsage?.gpu_devices ||
              currentUsage.gpu_devices.length === 0 ? (
                <div className="flex-1 min-w-80">
                  <Card title="GPU Usage" data-testid="hardware-gpu-empty-card">
                    <Text type="secondary">No GPU usage data available</Text>
                  </Card>
                </div>
              ) : (
                currentUsage.gpu_devices.map((gpuUsage, index) => {
                  // Find corresponding GPU info
                  const gpuInfo = hardwareInfo?.gpu_devices.find(
                    gpu => gpu.device_id === gpuUsage.device_id,
                  )

                  const gpuName =
                    gpuInfo?.name || gpuUsage.device_name || `GPU ${index + 1}`

                  return (
                    <div key={index} className="flex-1 min-w-80">
                      <Card title={`${gpuName} Usage`} data-testid={`hardware-gpu-card-${index}`}>
                        <div className="space-y-3">
                          {gpuUsage.utilization_percentage !== undefined && (
                            <div>
                              <Text strong>GPU Utilization</Text>
                              <Progress
                                data-testid={`hardware-gpu-util-progress-${index}`}
                                value={gpuUsage.utilization_percentage}
                                tone={gpuUsage.utilization_percentage > 90 ? 'error' : 'primary'}
                                aria-label="GPU utilization"
                                format={percent =>
                                  `${percent != null ? percent.toFixed(1) : '0.0'}%`
                                }
                              />
                            </div>
                          )}

                          {(gpuUsage.memory_usage_percentage !== undefined ||
                            (gpuUsage.memory_used !== undefined &&
                              gpuUsage.memory_total !== undefined)) && (
                            <div>
                              <Text strong>
                                {gpuInfo?.vendor?.includes('Apple')
                                  ? 'System Memory Usage'
                                  : 'GPU Memory'}
                              </Text>
                              {gpuUsage.memory_usage_percentage !==
                              undefined ? (
                                <Progress
                                  data-testid={`hardware-gpu-mem-progress-${index}`}
                                  value={gpuUsage.memory_usage_percentage}
                                  tone={gpuUsage.memory_usage_percentage > 90 ? 'error' : 'primary'}
                                  aria-label="GPU memory usage"
                                  format={percent =>
                                    `${percent != null ? percent.toFixed(1) : '0.0'}%`
                                  }
                                />
                              ) : (
                                gpuUsage.memory_used !== undefined &&
                                gpuUsage.memory_total !== undefined && (
                                  <Progress
                                    data-testid={`hardware-gpu-mem-progress-${index}`}
                                    value={(gpuUsage.memory_used / gpuUsage.memory_total) * 100}
                                    tone={(gpuUsage.memory_used / gpuUsage.memory_total) * 100 > 90 ? 'error' : 'primary'}
                                    aria-label="GPU memory usage"
                                    format={percent =>
                                      `${percent != null ? percent.toFixed(1) : '0.0'}%`
                                    }
                                  />
                                )
                              )}

                              {gpuUsage.memory_used !== undefined &&
                                gpuUsage.memory_total !== undefined && (
                                  <div className="mt-1">
                                    <Text type="secondary" className="text-xs">
                                      {gpuInfo?.vendor?.includes('Apple')
                                        ? 'GPU Memory Used: '
                                        : 'Used: '}
                                      {formatBytes(gpuUsage.memory_used)}
                                      {gpuInfo?.vendor?.includes('Apple')
                                        ? ` of ${formatBytes(gpuUsage.memory_total)} system memory`
                                        : ` / ${formatBytes(gpuUsage.memory_total)}`}
                                    </Text>
                                  </div>
                                )}
                            </div>
                          )}

                          <div className="flex gap-3">
                            {gpuUsage.temperature !== undefined && (
                              <Text type="secondary" className="text-xs">
                                Temperature: {gpuUsage.temperature}°C
                              </Text>
                            )}
                            {gpuUsage.power_usage !== undefined && (
                              <Text type="secondary" className="text-xs">
                                Power: {gpuUsage.power_usage}W
                              </Text>
                            )}
                          </div>
                        </div>
                      </Card>
                    </div>
                  )
                })
              )}
            </div>
          </>
        ) : (
          <Card data-testid="hardware-waiting-card">
            <div className="text-center py-8">
              <Text type="secondary">
                {sseConnected
                  ? 'Waiting for usage data...'
                  : 'Connect to hardware monitoring to view real-time usage data'}
              </Text>
            </div>
          </Card>
        )}
        </div>
      </div>
    </DivScrollY>
  )
}

import {
  Button,
  Card,
  ErrorState,
  Progress,
  Spin,
  Statistic,
  Tag,
  Text,
  message,
} from '@ziee/kit'
import { useEffect } from 'react'
import { Loading } from '@/core/components/Loading'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import { formatBytes } from '@/modules/hardware/utils/formatBytes'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { HardwareMonitorButton } from '@/modules/hardware/HardwareMonitorButton'
import { Hardware as HardwareStore } from '@/modules/hardware/hardware'

// A standalone copy button (reuses the kit Text `copyable` affordance with no
// content) placed next to a diagnostic value, so operators can lift model
// strings / driver versions / kernel names straight into a bug report. Renders
// nothing for empty / unknown values (nothing worth copying).
function CopyValue({ text, label, testId }: { text?: string | null; label: string; testId: string }) {
  if (!text || text === 'Unknown') return null
  return <Text copyable={{ text, label, testId }} />
}

export default function HardwareSettings() {
  const {
    hardwareInfo,
    hardwareLoading,
    hardwareError,
    currentUsage,
    usageLoading,
    sseConnected,
  } = HardwareStore

  const canMonitor = usePermission(Permissions.HardwareMonitor)

  // Initialize hardware monitoring on component mount. Only auto-connect
  // when the viewer has the monitor permission — non-monitor users see
  // the static hardware info card without a live SSE stream and without
  // a Connect/Monitor button.
  useEffect(() => {
    if (!canMonitor) return

    HardwareStore.subscribeToHardwareUsage().catch(console.error)

    // Cleanup on component unmount
    return () => {
      HardwareStore.disconnectHardwareUsage()
    }
  }, [canMonitor])

  // Live-monitoring transport state (usage/SSE) is surfaced persistently by
  // the connection-status card below — NOT by raw-string toasts. A cold
  // hardware-info load failure is shown as the in-place ErrorState below.
  // (The previous effect toasted all three raw error strings, double-signalling
  // the load failure and leaking transport state as user copy.)

  if (hardwareLoading) {
    return (
      <SettingsPageContainer title="Hardware">
        <Loading tip="Loading hardware information..." />
      </SettingsPageContainer>
    )
  }

  if (hardwareError && !hardwareInfo) {
    return (
      <SettingsPageContainer title="Hardware">
        <ErrorState
          resource="hardware information"
          description="Your hardware information couldn't be loaded. Check your connection and try again."
          details={hardwareError}
          onRetry={() => void HardwareStore.loadHardwareInfo()}
          data-testid="hardware-settings-error"
        />
      </SettingsPageContainer>
    )
  }

  const renderOperatingSystemCard = () => (
    <Card title="Operating System" data-testid="hardware-os-card">
      <div className="flex flex-wrap gap-6">
        <Statistic
          data-testid="hardware-os-name"
          title="Name"
          value={hardwareInfo?.operating_system.name || 'Unknown'}
          suffix={<CopyValue text={hardwareInfo?.operating_system.name} label="Copy OS name" testId="hardware-os-name-copy" />}
        />
        <Statistic
          data-testid="hardware-os-version"
          title="Version"
          value={hardwareInfo?.operating_system.version || 'Unknown'}
          suffix={<CopyValue text={hardwareInfo?.operating_system.version} label="Copy OS version" testId="hardware-os-version-copy" />}
        />
        <Statistic
          data-testid="hardware-os-arch"
          title="Architecture"
          value={hardwareInfo?.operating_system.architecture || 'Unknown'}
        />
        {hardwareInfo?.operating_system.kernel_version && (
          <Statistic
            data-testid="hardware-os-kernel"
            title="Kernel"
            value={hardwareInfo.operating_system.kernel_version}
            suffix={<CopyValue text={hardwareInfo.operating_system.kernel_version} label="Copy kernel version" testId="hardware-os-kernel-copy" />}
          />
        )}
      </div>
    </Card>
  )

  const renderCPUCard = () => (
    <Card title="CPU" data-testid="hardware-cpu-info-card">
      <div className="flex flex-col gap-4">
        <div className="flex flex-wrap gap-6">
          <Statistic
            data-testid="hardware-cpu-model"
            title="Model"
            value={hardwareInfo?.cpu.model || 'Unknown'}
            suffix={<CopyValue text={hardwareInfo?.cpu.model} label="Copy CPU model" testId="hardware-cpu-model-copy" />}
          />
          <Statistic
            data-testid="hardware-cpu-arch"
            title="Architecture"
            value={hardwareInfo?.cpu.architecture || 'Unknown'}
          />
          <Statistic data-testid="hardware-cpu-cores" title="Cores" value={hardwareInfo?.cpu.cores || 0} />
          {hardwareInfo?.cpu.threads && (
            <Statistic data-testid="hardware-cpu-threads" title="Threads" value={hardwareInfo.cpu.threads} />
          )}
          {hardwareInfo?.cpu.base_frequency && (
            <Statistic
              data-testid="hardware-cpu-base-freq"
              title="Base Frequency"
              value={`${hardwareInfo.cpu.base_frequency} MHz`}
            />
          )}
          {hardwareInfo?.cpu.max_frequency && (
            <Statistic
              data-testid="hardware-cpu-max-freq"
              title="Max Frequency"
              value={`${hardwareInfo.cpu.max_frequency} MHz`}
            />
          )}
        </div>
        {currentUsage && (
          <div>
            <Text strong>CPU Usage</Text>
            <Progress
              data-testid="hardware-cpu-usage-progress"
              value={currentUsage.cpu.usage_percentage}
              tone={currentUsage.cpu.usage_percentage > 90 ? 'error' : 'primary'}
              format={percent =>
                `${percent != null ? percent.toFixed(1) : '0.0'}%`
              }
              aria-label="CPU usage"
            />
            <div className="flex gap-3 mt-2">
              {currentUsage.cpu.temperature && (
                <Text type="secondary" className="text-xs">
                  Temperature: {currentUsage.cpu.temperature}°C
                </Text>
              )}
              {currentUsage.cpu.frequency && (
                <Text type="secondary" className="text-xs">
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
    <Card title="Memory" data-testid="hardware-memory-info-card">
      <div className="flex flex-col gap-4">
        <div className="flex flex-wrap gap-6">
          <div>
            <Text type="secondary" className="text-xs block">
              Total RAM
            </Text>
            <div className="text-2xl font-semibold">
              {formatBytes(hardwareInfo?.memory.total_ram || 0)}
            </div>
          </div>
          {hardwareInfo?.memory.total_swap !== undefined &&
            hardwareInfo.memory.total_swap > 0 && (
              <div>
                <Text type="secondary" className="text-xs block">
                  Total Swap
                </Text>
                <div className="text-2xl font-semibold">
                  {formatBytes(hardwareInfo.memory.total_swap)}
                </div>
              </div>
            )}
        </div>
        {currentUsage && (
          <div>
            <Text strong>Memory Usage</Text>
            <Progress
              data-testid="hardware-memory-usage-progress"
              value={currentUsage.memory.usage_percentage}
              tone={currentUsage.memory.usage_percentage > 90 ? 'error' : 'primary'}
              format={percent =>
                `${percent != null ? percent.toFixed(1) : '0.0'}%`
              }
              aria-label="Memory usage"
            />
            <div className="flex gap-3 mt-2">
              <Text type="secondary" className="text-xs">
                Used: {formatBytes(currentUsage.memory.used_ram)}
              </Text>
              <Text type="secondary" className="text-xs">
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
        <Card title="GPU" data-testid="hardware-gpu-none-card">
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

      // Prefix a slot index ("GPU 0 · <name>") only on multi-GPU hosts, so two
      // identically-named cards (e.g. 2× "NVIDIA H200") are distinguishable at a
      // glance; a single-GPU box keeps the bare device name.
      const multiGpu = hardwareInfo.gpu_devices.length > 1
      const cardTitle = multiGpu ? `GPU ${index} · ${gpu.name}` : gpu.name

      return (
        <Card key={index} title={cardTitle} data-testid={`hardware-gpu-info-card-${index}`}>
          <div className="flex flex-col gap-4">
            <div className="flex flex-wrap gap-6">
              <div>
                <Text type="secondary" className="text-xs block">
                  Vendor
                </Text>
                <div className="flex items-baseline gap-1 text-2xl font-semibold">
                  {gpu.vendor}
                  <CopyValue text={gpu.vendor} label="Copy GPU vendor" testId={`hardware-gpu-vendor-copy-${index}`} />
                </div>
              </div>
              {gpu.memory ? (
                <div>
                  <Text type="secondary" className="text-xs block">
                    {gpu.vendor?.includes('Apple') ? 'Dedicated VRAM' : 'Memory'}
                  </Text>
                  <div className="text-2xl font-semibold">
                    {formatBytes(gpu.memory)}
                  </div>
                </div>
              ) : gpu.vendor?.includes('Apple') ? (
                <div>
                  <Text type="secondary" className="text-xs block">
                    Memory Architecture
                  </Text>
                  <div className="text-2xl font-semibold">Unified Memory</div>
                </div>
              ) : null}
              {gpu.driver_version && (
                <div>
                  <Text type="secondary" className="text-xs block">
                    Driver
                  </Text>
                  <div className="flex items-baseline gap-1 text-2xl font-semibold">
                    {gpu.driver_version}
                    <CopyValue text={gpu.driver_version} label="Copy GPU driver version" testId={`hardware-gpu-driver-copy-${index}`} />
                  </div>
                </div>
              )}
              {gpu.vendor?.includes('Apple') && hardwareInfo?.memory && (
                <div>
                  <Text type="secondary" className="text-xs block">
                    Shared System Memory
                  </Text>
                  <div className="text-2xl font-semibold">
                    {formatBytes(hardwareInfo.memory.total_ram)}
                  </div>
                </div>
              )}
            </div>

            <div>
              <Text strong className="mb-2 block">
                Compute Support
              </Text>
              <div className="flex flex-wrap gap-1">
                <Tag variant="outline"
                  data-testid={`hardware-gpu-cuda-tag-${index}`}
                  tone={gpu.compute_capabilities.cuda_support ? 'success' : 'default'}
                >
                  CUDA {gpu.compute_capabilities.cuda_support ? '✓' : '✗'}
                  {gpu.compute_capabilities.cuda_version &&
                    ` (${gpu.compute_capabilities.cuda_version})`}
                </Tag>
                <Tag variant="outline"
                  data-testid={`hardware-gpu-metal-tag-${index}`}
                  tone={gpu.compute_capabilities.metal_support ? 'success' : 'default'}
                >
                  Metal {gpu.compute_capabilities.metal_support ? '✓' : '✗'}
                </Tag>
                <Tag variant="outline"
                  data-testid={`hardware-gpu-opencl-tag-${index}`}
                  tone={gpu.compute_capabilities.opencl_support ? 'success' : 'default'}
                >
                  OpenCL {gpu.compute_capabilities.opencl_support ? '✓' : '✗'}
                </Tag>
                {gpu.compute_capabilities.vulkan_support !== undefined && (
                  <Tag variant="outline"
                    data-testid={`hardware-gpu-vulkan-tag-${index}`}
                    tone={gpu.compute_capabilities.vulkan_support ? 'success' : 'default'}
                  >
                    Vulkan {gpu.compute_capabilities.vulkan_support ? '✓' : '✗'}
                  </Tag>
                )}
              </div>
            </div>

            {gpuUsage && (
              <div>
                {gpuUsage.utilization_percentage !== undefined && (
                  <div className="mb-3">
                    <Text strong>GPU Utilization</Text>
                    <Progress
                      data-testid={`hardware-gpu-util-progress-${index}`}
                      value={gpuUsage.utilization_percentage}
                      tone={gpuUsage.utilization_percentage > 90 ? 'error' : 'primary'}
                      format={percent =>
                        `${percent != null ? percent.toFixed(1) : '0.0'}%`
                      }
                      aria-label="GPU utilization"
                    />
                  </div>
                )}

                {(gpuUsage.memory_usage_percentage !== undefined ||
                  (gpuUsage.memory_used !== undefined &&
                    gpuUsage.memory_total !== undefined)) && (
                  <div className="mb-3">
                    <Text strong>
                      {gpu.vendor?.includes('Apple') ? 'System Memory Usage' : 'GPU Memory'}
                    </Text>
                    {gpuUsage.memory_usage_percentage !== undefined ? (
                      <Progress
                        data-testid={`hardware-gpu-mem-progress-${index}`}
                        value={gpuUsage.memory_usage_percentage}
                        tone={gpuUsage.memory_usage_percentage > 90 ? 'error' : 'primary'}
                        format={percent =>
                          `${percent != null ? percent.toFixed(1) : '0.0'}%`
                        }
                        aria-label="GPU memory usage"
                      />
                    ) : gpuUsage.memory_used !== undefined &&
                      gpuUsage.memory_total !== undefined ? (
                      <Progress
                        data-testid={`hardware-gpu-mem-progress-${index}`}
                        value={
                          (gpuUsage.memory_used / gpuUsage.memory_total) * 100
                        }
                        tone={
                          (gpuUsage.memory_used / gpuUsage.memory_total) * 100 > 90
                            ? 'error'
                            : 'primary'
                        }
                        format={percent =>
                          `${percent != null ? percent.toFixed(1) : '0.0'}%`
                        }
                        aria-label="GPU memory usage"
                      />
                    ) : null}

                    {gpuUsage.memory_used !== undefined &&
                      gpuUsage.memory_total !== undefined && (
                        <div className="mt-1">
                          <Text type="secondary" className="text-xs block">
                            {gpu.vendor?.includes('Apple')
                              ? 'GPU Memory Used: '
                              : 'Used: '}
                            {formatBytes(gpuUsage.memory_used)}
                            {gpu.vendor?.includes('Apple')
                              ? ` of ${formatBytes(gpuUsage.memory_total)} total system memory`
                              : ` / ${formatBytes(gpuUsage.memory_total)}`}
                          </Text>
                          {gpu.vendor?.includes('Apple') && (
                            <Text type="secondary" className="text-xs italic block">
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
                    <div className="mb-3">
                      <Text strong className="block mb-2">
                        Real-time Statistics
                      </Text>
                      <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
                        {gpuUsage.utilization_percentage !== undefined && (
                          <div>
                            <Text type="secondary" className="text-xs">
                              GPU Usage
                            </Text>
                            <div className="text-sm font-bold">
                              {gpuUsage.utilization_percentage != null
                                ? gpuUsage.utilization_percentage.toFixed(1)
                                : '0.0'}
                              %
                            </div>
                          </div>
                        )}
                        {gpuUsage.memory_usage_percentage !== undefined && (
                          <div>
                            <Text type="secondary" className="text-xs">
                              Memory Usage
                            </Text>
                            <div className="text-sm font-bold">
                              {gpuUsage.memory_usage_percentage != null
                                ? gpuUsage.memory_usage_percentage.toFixed(1)
                                : '0.0'}
                              %
                            </div>
                          </div>
                        )}
                        {gpuUsage.memory_used !== undefined && (
                          <div>
                            <Text type="secondary" className="text-xs">
                              Memory Used
                            </Text>
                            <div className="text-sm font-bold">
                              {formatBytes(gpuUsage.memory_used)}
                            </div>
                          </div>
                        )}
                        {gpuUsage.temperature !== undefined && (
                          <div>
                            <Text type="secondary" className="text-xs">
                              Temperature
                            </Text>
                            <div className="text-sm font-bold">
                              {gpuUsage.temperature}°C
                            </div>
                          </div>
                        )}
                        {gpuUsage.power_usage !== undefined && (
                          <div>
                            <Text type="secondary" className="text-xs">
                              Power Draw
                            </Text>
                            <div className="text-sm font-bold">
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
                {/* Temperature & Power are already shown in the Real-time
                    Statistics grid above; the previous trailing row here
                    duplicated them and has been removed. */}
              </div>
            )}
          </div>
        </Card>
      )
    })
  }

  const handleManualConnect = async () => {
    try {
      await HardwareStore.subscribeToHardwareUsage()
      message.success('Connecting to hardware monitoring...')
    } catch (_error) {
      message.error('Failed to connect to hardware monitoring')
    }
  }

  const renderConnectionStatus = () => (
    <Card className={sseConnected ? 'hidden' : 'block'} data-testid="hardware-settings-connection-card">
      <div className="flex items-center gap-3 flex-wrap">
        <div className="flex gap-3 flex-wrap">
          <Text strong>Real-time Monitoring:</Text>
          <Tag variant="outline" data-testid="hardware-settings-connection-tag" tone={sseConnected ? 'success' : 'error'}>
            {sseConnected ? 'Connected' : 'Disconnected'}
          </Tag>
        </div>
        {canMonitor && !sseConnected && !usageLoading && (
          <Button data-testid="hardware-settings-connect-btn" variant="default" onClick={handleManualConnect}>
            Connect
          </Button>
        )}
        {usageLoading && (
          <div className="flex items-center gap-2">
            <Spin label="Connecting" />
            <Text type="secondary">Connecting...</Text>
          </div>
        )}
        {currentUsage && (
          <Text type="secondary" className="text-xs">
            Last update: {new Date(currentUsage.timestamp).toLocaleTimeString()}
          </Text>
        )}
      </div>
    </Card>
  )

  const titleWithButton = (
    <div className="flex items-center justify-between w-full">
      <span>Hardware</span>
      {/* Extracted so desktop can override via localOverridePlugin
        * — desktop opens a native Tauri window instead of a browser
        * popup. Self-permission-gated (returns null without
        * `hardware::monitor`). */}
      <HardwareMonitorButton />
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

import { useEffect, useState } from 'react'
import {
  Alert,
  Button,
  Card,
  Col,
  Divider,
  Flex,
  Form,
  Input,
  InputNumber,
  Row,
  Spin,
  Typography,
  message,
} from 'antd'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import {
  Permissions,
  type CodeSandboxResourceLimits,
  type UpdateCodeSandboxResourceLimits,
} from '@/api-client/types'

const MANAGE_PERM = Permissions.CodeSandboxResourceLimitsManage
const READ_PERM = Permissions.CodeSandboxResourceLimitsRead

const MIB = 1024 * 1024
const GIB = 1024 * 1024 * 1024

/** Form values mirror the API row but make the byte fields editable in MiB. */
type FormValues = {
  memory_max_mib: number
  memory_swap_max_mib: number
  pids_max: number
  cpu_max: string
  address_space_mib: number
  fsize_mib: number
  nproc_max: number
  nofile_max: number
  cpu_secs_max: number
  timeout_secs: number
  vm_idle_evict_secs: number
  mac_vm_vcpus: number
  mac_vm_ram_mib: number
  vm_max_concurrent_execs: number
}

function rowToForm(row: CodeSandboxResourceLimits): FormValues {
  return {
    memory_max_mib: Math.round(row.memory_max_bytes / MIB),
    memory_swap_max_mib: Math.round(row.memory_swap_max_bytes / MIB),
    pids_max: row.pids_max,
    cpu_max: row.cpu_max,
    address_space_mib: Math.round(row.address_space_bytes / MIB),
    fsize_mib: Math.round(row.fsize_bytes / MIB),
    nproc_max: row.nproc_max,
    nofile_max: row.nofile_max,
    cpu_secs_max: row.cpu_secs_max,
    timeout_secs: row.timeout_secs,
    vm_idle_evict_secs: row.vm_idle_evict_secs,
    mac_vm_vcpus: row.mac_vm_vcpus,
    mac_vm_ram_mib: row.mac_vm_ram_mib,
    vm_max_concurrent_execs: row.vm_max_concurrent_execs,
  }
}

function formToPatch(v: FormValues): UpdateCodeSandboxResourceLimits {
  return {
    memory_max_bytes: v.memory_max_mib * MIB,
    memory_swap_max_bytes: v.memory_swap_max_mib * MIB,
    pids_max: v.pids_max,
    cpu_max: v.cpu_max,
    address_space_bytes: v.address_space_mib * MIB,
    fsize_bytes: v.fsize_mib * MIB,
    nproc_max: v.nproc_max,
    nofile_max: v.nofile_max,
    cpu_secs_max: v.cpu_secs_max,
    timeout_secs: v.timeout_secs,
    vm_idle_evict_secs: v.vm_idle_evict_secs,
    mac_vm_vcpus: v.mac_vm_vcpus,
    mac_vm_ram_mib: v.mac_vm_ram_mib,
    vm_max_concurrent_execs: v.vm_max_concurrent_execs,
  }
}

/**
 * Resource-limits admin section. Rendered as a sequence of `<Card>` groups
 * inside the parent `SandboxSettingsPage`. Owns the singleton-row Form +
 * Save/Reset controls. Permission-aware: when the viewer lacks
 * `code_sandbox::resource_limits::manage`, the form goes read-only and Save
 * is disabled (read access is implicit via the surrounding container).
 */
export function SandboxResourceLimitsSection() {
  const { limits, loading, saving, error } = Stores.SandboxResourceLimits
  const canManage = usePermission(MANAGE_PERM)
  const canRead = usePermission(READ_PERM) || canManage

  const [form] = Form.useForm<FormValues>()
  const [dirty, setDirty] = useState(false)

  // Sync the form whenever the loaded row changes (initial load, save reply).
  useEffect(() => {
    if (limits) {
      form.setFieldsValue(rowToForm(limits))
      setDirty(false)
    }
  }, [limits, form])

  const onSubmit = async (v: FormValues) => {
    try {
      await Stores.SandboxResourceLimits.saveLimits(formToPatch(v))
      message.success('Resource limits saved')
    } catch (e: any) {
      message.error(e?.message ?? 'Failed to save')
    }
  }

  const onReset = () => {
    if (limits) {
      form.setFieldsValue(rowToForm(limits))
      setDirty(false)
    }
  }

  if (!canRead) {
    return (
      <Card title="Resource limits">
        <Alert
          type="warning"
          showIcon
          title="You don't have permission to view sandbox resource limits."
        />
      </Card>
    )
  }

  return (
    <>
      {error && (
        <Alert
          type="error"
          title="Failed to load resource limits"
          description={error}
          showIcon
                 />
      )}

      {loading && !limits ? (
        <Spin description="Loading resource limits…" />
      ) : (
        <Card title="Resource limits">
        <Form
          form={form}
          layout="vertical"
          onFinish={onSubmit}
          onValuesChange={() => setDirty(true)}
          disabled={!canManage}
        >
          {!canManage && (
            <Alert
              type="info"
              title="Read-only view"
              description="You have read permission for resource limits but not manage. Save is disabled."
              showIcon
                         />
          )}

          {/* Flat form. Section headers via Divider (no sub-cards)
            * so the form reads as one cohesive surface, matching
            * the visual rhythm of HardwareSettings / MemoryAdmin
            * forms. */}
          <Divider titlePlacement="start" styles={{ content: { margin: 0 } }}>
            <Typography.Text type="secondary" className="text-xs">
              Memory
            </Typography.Text>
          </Divider>
          <Row gutter={16}>
            <Col span={8}>
              <Form.Item
                name="memory_max_mib"
                label="memory.max"
                extra="cgroup v2 memory cap (MiB). OOM-kills the workload if exceeded."
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 16
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be ≥ 16 MiB')),
                  },
                ]}
              >
                <InputNumber min={16} suffix="MiB" style={{ width: '100%' }} />
              </Form.Item>
            </Col>
            <Col span={8}>
              <Form.Item
                name="memory_swap_max_mib"
                label="memory.swap.max"
                extra="cgroup v2 swap cap (MiB). 0 disables swap."
              >
                <InputNumber min={0} suffix="MiB" style={{ width: '100%' }} />
              </Form.Item>
            </Col>
            <Col span={8}>
              <Form.Item
                name="address_space_mib"
                label="rlimit --as"
                extra="Virtual address space cap (MiB). prlimit backstop."
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 16
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be ≥ 16 MiB')),
                  },
                ]}
              >
                <InputNumber min={16} suffix="MiB" style={{ width: '100%' }} />
              </Form.Item>
            </Col>
          </Row>

          <Divider titlePlacement="start" styles={{ content: { margin: 0 } }}>
            <Typography.Text type="secondary" className="text-xs">
              Processes &amp; CPU
            </Typography.Text>
          </Divider>
          <Row gutter={16}>
            <Col span={8}>
              <Form.Item
                name="pids_max"
                label="cgroup pids.max"
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 8 && v <= 100_000
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be 8..=100000')),
                  },
                ]}
              >
                <InputNumber min={8} max={100_000} style={{ width: '100%' }} />
              </Form.Item>
            </Col>
            <Col span={8}>
              <Form.Item
                name="nproc_max"
                label="rlimit --nproc"
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 8 && v <= 100_000
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be 8..=100000')),
                  },
                ]}
              >
                <InputNumber min={8} max={100_000} style={{ width: '100%' }} />
              </Form.Item>
            </Col>
            <Col span={8}>
              <Form.Item
                name="cpu_max"
                label="cgroup cpu.max"
                extra='"<quota_us> <period_us>" — "100000 100000" = 1 CPU'
                rules={[
                  {
                    pattern: /^[0-9]+ [0-9]+$/,
                    message: 'shape: "<quota> <period>" (digits)',
                  },
                ]}
              >
                <Input placeholder="100000 100000" />
              </Form.Item>
            </Col>
          </Row>
          <Row gutter={16}>
            <Col span={12}>
              <Form.Item
                name="cpu_secs_max"
                label="rlimit --cpu (seconds)"
                extra="CPU-seconds backstop. Largely redundant with the wall-clock timeout."
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 10 && v <= 86_400
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be 10..=86400')),
                  },
                ]}
              >
                <InputNumber min={10} max={86_400} suffix="s" style={{ width: '100%' }} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item
                name="timeout_secs"
                label="Wall-clock per-exec timeout"
                extra="Hard SIGKILL after this many seconds."
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 5 && v <= 86_400
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be 5..=86400')),
                  },
                ]}
              >
                <InputNumber min={5} max={86_400} suffix="s" style={{ width: '100%' }} />
              </Form.Item>
            </Col>
          </Row>

          <Divider titlePlacement="start" styles={{ content: { margin: 0 } }}>
            <Typography.Text type="secondary" className="text-xs">
              Files &amp; descriptors
            </Typography.Text>
          </Divider>
          <Row gutter={16}>
            <Col span={12}>
              <Form.Item
                name="fsize_mib"
                label="rlimit --fsize (single file)"
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 1
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be ≥ 1 MiB')),
                  },
                ]}
              >
                <InputNumber min={1} suffix="MiB" style={{ width: '100%' }} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item
                name="nofile_max"
                label="rlimit --nofile"
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 64 && v <= 1_048_576
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be 64..=1048576')),
                  },
                ]}
              >
                <InputNumber min={64} max={1_048_576} style={{ width: '100%' }} />
              </Form.Item>
            </Col>
          </Row>

          <Divider titlePlacement="start" styles={{ content: { margin: 0 } }}>
            <Typography.Text type="secondary" className="text-xs">
              VM lifecycle (macOS + Windows)
            </Typography.Text>
          </Divider>
          <Row gutter={16}>
            <Col span={12}>
              <Form.Item
                name="vm_idle_evict_secs"
                label="Idle-evict timeout"
                extra="After this many idle seconds with nothing in-flight, the per-flavor microVM / WSL2 distro is evicted to free its RAM. Set to 0 to disable eviction (warm VMs hold memory indefinitely)."
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 0
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be ≥ 0')),
                  },
                ]}
              >
                <InputNumber min={0} suffix="s (0 = never)" style={{ width: '100%' }} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item
                name="vm_max_concurrent_execs"
                label="Concurrent execs per VM / distro"
                extra="Cap on parallel execute_command calls that share one VM. Each call is cgroup-capped at memory.max; this bound keeps N concurrent calls from summing past the VM's RAM ceiling. Applies to macOS libkrun + Windows WSL2."
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 1 && v <= 1000
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be in 1..=1000')),
                  },
                ]}
              >
                <InputNumber min={1} max={1000} style={{ width: '100%' }} />
              </Form.Item>
            </Col>
          </Row>

          <Divider titlePlacement="start" styles={{ content: { margin: 0 } }}>
            <Typography.Text type="secondary" className="text-xs">
              macOS libkrun VM sizing
            </Typography.Text>
          </Divider>
          <Row gutter={16}>
            <Col span={12}>
              <Form.Item
                name="mac_vm_vcpus"
                label="vCPUs"
                extra="Per-flavor libkrun microVM vCPU count (krun_set_vm_config num_vcpus). Applies on the NEXT cold boot of a flavor; warm VMs keep their boot-time sizing."
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 1 && v <= 128
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be in 1..=128')),
                  },
                ]}
              >
                <InputNumber min={1} max={128} style={{ width: '100%' }} />
              </Form.Item>
            </Col>
            <Col span={12}>
              <Form.Item
                name="mac_vm_ram_mib"
                label="RAM ceiling"
                extra="Per-flavor libkrun microVM RAM ceiling in MiB. Host RAM is demand-paged; this is the upper bound. Applies on the NEXT cold boot of a flavor."
                rules={[
                  {
                    validator: (_, v) =>
                      v >= 256 && v <= 262_144
                        ? Promise.resolve()
                        : Promise.reject(new Error('must be in 256..=262144 MiB')),
                  },
                ]}
              >
                <InputNumber min={256} max={262_144} suffix="MiB" style={{ width: '100%' }} />
              </Form.Item>
            </Col>
          </Row>

          {/* Actions on the right — matches the pattern used by the
            * Memory admin / PreferencesSection forms. */}
          <Flex justify="end" gap="small" className="mt-3">
            <Button onClick={onReset} disabled={!dirty || saving}>
              Reset
            </Button>
            <Button
              type="primary"
              htmlType="submit"
              loading={saving}
              disabled={!canManage || !dirty}
            >
              Save
            </Button>
          </Flex>

          <Typography.Paragraph type="secondary" style={{ marginTop: 24 }}>
            Defaults: 512 MiB memory, 256 PIDs, 1 CPU, 4 GiB address space,
            256 MiB single-file, 1024 nofile, 1240 s CPU-seconds backstop,
            620 s wall-clock, 900 s VM idle-evict. Values stored at{' '}
            <code>code_sandbox_settings</code>; the server invalidates its
            in-process cache on save.
          </Typography.Paragraph>
        </Form>
        </Card>
      )}
    </>
  )
}

// Marker to silence the `GIB` unused-import warning while keeping the constant
// available for future inputs that prefer GiB display granularity.
void GIB

import { useEffect, useState } from 'react'
import {
  Alert,
  Button,
  Card,
  Col,
  Form,
  Input,
  InputNumber,
  Row,
  Space,
  Spin,
  Typography,
  message,
} from 'antd'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { Stores } from '@/core/stores'
import type {
  CodeSandboxResourceLimits,
  UpdateCodeSandboxResourceLimits,
} from '@/api-client/types'

const MANAGE_PERM = 'code_sandbox::resource_limits::manage'

/**
 * Permission predicate aligned with the backend's `has_permission`
 * (auth/backend.rs): honor the global `*` wildcard + `resource:*` wildcard
 * in addition to exact matches. Without this, an Administrator (seeded with
 * `['*']`) would see a forbidden message.
 */
function hasPermission(perms: string[], perm: string): boolean {
  if (perms.includes('*')) return true
  if (perms.includes(perm)) return true
  const idx = perm.indexOf(':')
  if (idx > 0 && perms.includes(`${perm.slice(0, idx)}:*`)) return true
  return false
}

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
  }
}

export function SandboxResourceLimitsPage() {
  const { limits, loading, saving, error } = Stores.SandboxResourceLimits
  const { permissions } = Stores.Auth
  const perms = permissions ?? []
  const canManage = hasPermission(perms, MANAGE_PERM)

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

  return (
    <SettingsPageContainer
      title="Sandbox resource limits"
      subtitle="cgroup + prlimit caps applied to every code_sandbox execute_command. Changes take effect immediately — the server invalidates its in-process cache on save."
    >
      {error && (
        <Alert
          type="error"
          message="Failed to load resource limits"
          description={error}
          showIcon
          style={{ marginBottom: 16 }}
        />
      )}

      {loading && !limits ? (
        <Spin tip="Loading resource limits…" />
      ) : (
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
              message="Read-only view"
              description="You have read permission for resource limits but not manage. Save is disabled."
              showIcon
              style={{ marginBottom: 16 }}
            />
          )}

          <Card title="Memory" size="small" style={{ marginBottom: 16 }}>
            <Row gutter={16}>
              <Col span={8}>
                <Form.Item
                  name="memory_max_mib"
                  label="memory.max"
                  tooltip="cgroup v2 memory cap (MiB). OOM-kills the workload if exceeded."
                  rules={[
                    {
                      validator: (_, v) =>
                        v >= 16
                          ? Promise.resolve()
                          : Promise.reject(new Error('must be ≥ 16 MiB')),
                    },
                  ]}
                >
                  <InputNumber min={16} addonAfter="MiB" style={{ width: '100%' }} />
                </Form.Item>
              </Col>
              <Col span={8}>
                <Form.Item
                  name="memory_swap_max_mib"
                  label="memory.swap.max"
                  tooltip="cgroup v2 swap cap (MiB). 0 disables swap."
                >
                  <InputNumber min={0} addonAfter="MiB" style={{ width: '100%' }} />
                </Form.Item>
              </Col>
              <Col span={8}>
                <Form.Item
                  name="address_space_mib"
                  label="rlimit --as"
                  tooltip="Virtual address space cap (MiB). prlimit backstop."
                  rules={[
                    {
                      validator: (_, v) =>
                        v >= 16
                          ? Promise.resolve()
                          : Promise.reject(new Error('must be ≥ 16 MiB')),
                    },
                  ]}
                >
                  <InputNumber min={16} addonAfter="MiB" style={{ width: '100%' }} />
                </Form.Item>
              </Col>
            </Row>
          </Card>

          <Card title="Processes + CPU" size="small" style={{ marginBottom: 16 }}>
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
                  tooltip='"<quota_us> <period_us>" — "100000 100000" = 1 CPU'
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
                  tooltip="CPU-seconds backstop. Largely redundant with the wall-clock timeout."
                  rules={[
                    {
                      validator: (_, v) =>
                        v >= 10 && v <= 86_400
                          ? Promise.resolve()
                          : Promise.reject(new Error('must be 10..=86400')),
                    },
                  ]}
                >
                  <InputNumber min={10} max={86_400} addonAfter="s" style={{ width: '100%' }} />
                </Form.Item>
              </Col>
              <Col span={12}>
                <Form.Item
                  name="timeout_secs"
                  label="Wall-clock per-exec timeout"
                  tooltip="Hard SIGKILL after this many seconds."
                  rules={[
                    {
                      validator: (_, v) =>
                        v >= 5 && v <= 86_400
                          ? Promise.resolve()
                          : Promise.reject(new Error('must be 5..=86400')),
                    },
                  ]}
                >
                  <InputNumber min={5} max={86_400} addonAfter="s" style={{ width: '100%' }} />
                </Form.Item>
              </Col>
            </Row>
          </Card>

          <Card title="Files + descriptors" size="small" style={{ marginBottom: 16 }}>
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
                  <InputNumber min={1} addonAfter="MiB" style={{ width: '100%' }} />
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
          </Card>

          <Card title="VM lifecycle (macOS + Windows)" size="small" style={{ marginBottom: 16 }}>
            <Row gutter={16}>
              <Col span={24}>
                <Form.Item
                  name="vm_idle_evict_secs"
                  label="Idle-evict timeout"
                  tooltip="After this many idle seconds with nothing in-flight, the per-flavor microVM / WSL2 distro is evicted to free its RAM. Set to 0 to disable eviction (warm VMs hold memory indefinitely)."
                  rules={[
                    {
                      validator: (_, v) =>
                        v >= 0
                          ? Promise.resolve()
                          : Promise.reject(new Error('must be ≥ 0')),
                    },
                  ]}
                >
                  <InputNumber min={0} addonAfter="s (0 = never)" style={{ width: '100%' }} />
                </Form.Item>
              </Col>
            </Row>
          </Card>

          <Space>
            <Button
              type="primary"
              htmlType="submit"
              loading={saving}
              disabled={!canManage || !dirty}
            >
              Save
            </Button>
            <Button onClick={onReset} disabled={!dirty || saving}>
              Reset
            </Button>
            <Typography.Text type="secondary">
              {limits ? `Last updated: ${new Date(limits.updated_at).toLocaleString()}` : ''}
            </Typography.Text>
          </Space>

          <Typography.Paragraph type="secondary" style={{ marginTop: 24 }}>
            Defaults: 512 MiB memory, 256 PIDs, 1 CPU, 4 GiB address space,
            256 MiB single-file, 1024 nofile, 1240 s CPU-seconds backstop,
            620 s wall-clock, 900 s VM idle-evict. Values stored at{' '}
            <code>code_sandbox_settings</code>; the server invalidates its
            in-process cache on save.
          </Typography.Paragraph>
        </Form>
      )}
    </SettingsPageContainer>
  )
}

// Marker to silence the `GIB` unused-import warning while keeping the constant
// available for future inputs that prefer GiB display granularity.
void GIB
